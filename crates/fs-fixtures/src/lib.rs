//! Checking the engine against Onshape's fixtures.
//!
//! A fixture batch (`fixtures/batch-NNN`) holds the case functions (`cases.fs`), the generator feature that built
//! them in Onshape (`generator.fs`) and Onshape's model (`onshape.step`). [`build`] runs the same generator on the
//! engine, so the engine builds every case in one context, moves it to its slot and names its parts, as Onshape
//! did. [`check_batch`] then compares each part with Onshape's part of the same name.
//!
//! The engine recurses deeply, so call these on a thread from [`fs_interp::run_with_large_stack`].

use fs_geom::model::BodyType;
use fs_interp::Value;
use occt::{BooleanKind, Box3, Shape, ShapeKind};
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::time::Instant;

/// Volume and area must match to this relative error, and the volume where the shapes differ must be at most
/// this fraction of the part's volume.
pub const RELATIVE_TOLERANCE: f64 = 1e-6;

/// Bounding box corners must match to this distance (metres). Onshape's curved parts read from STEP have boxes
/// 1e-7 m bigger all round (their tolerance), so this can't be tighter.
pub const BOX_TOLERANCE: f64 = 1e-6;

/// For the difference check, geometry closer than this (metres) counts as coincident. STEP stores coordinates to
/// about 15 significant figures, so faces both modellers put in the same place can be a few nanometres apart.
pub const FUZZY: f64 = 1e-7;

pub struct Part {
    pub name: String,
    pub shape: Shape,
}

pub struct Build {
    /// The named solid bodies, in model order.
    pub parts: Vec<Part>,
    /// What the generator printed.
    pub output: String,
    pub seconds: f64,
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn exception(e: fs_interp::Exception) -> String {
    match e.0.get("message") {
        Some(message) => format!("{message:?}"),
        None => e.to_string(),
    }
}

/// Runs a batch's generator feature on the engine, on a new context with the default planes.
pub fn build(batch: &Path) -> Result<Build, String> {
    // Onshape's Feature Studio held cases.fs followed by generator.fs.
    let mut generator = read(&batch.join("generator.fs"))?;
    // Diagnostic only: FS_ONLY=c001,c002 builds just those cases.
    if let Ok(only) = std::env::var("FS_ONLY") {
        generator = only_cases(&generator, &only.split(',').collect::<Vec<_>>());
    }
    let source = format!("{}\n{}", read(&batch.join("cases.fs"))?, generator);
    let mut engine = fs_geom::engine();
    engine.interp.trace_catches = std::env::var_os("FS_TRACE_CATCHES").is_some();
    let module = engine.load_source(&batch.join("feature-studio.fs").to_string_lossy(), &source)?;
    let generator = engine.interp.modules.list[module]
        .ast
        .items
        .iter()
        .find_map(|item| match &item.kind {
            fs_lang::ast::ItemKind::Const { name, .. } if name.name.starts_with("buildFixtureBatch") => {
                Some(name.name.clone())
            }
            _ => None,
        })
        .ok_or("generator.fs has no buildFixtureBatch feature")?;

    let start = Instant::now();
    let context = engine.eval(module, "newContextWithDefaults()").map_err(exception)?;
    let id = engine.eval(module, "makeId(\"Fgenerator\")").map_err(exception)?;
    let definition = Value::map(Default::default());
    engine.call(module, &generator, vec![context.clone(), id.clone(), definition]).map_err(exception)?;
    let seconds = start.elapsed().as_secs_f64();
    // A top-level feature reports its errors as its status instead of throwing.
    let status = engine.call(module, "getFeatureStatus", vec![context.clone(), id]).map_err(exception)?;
    if status.get("statusType").and_then(Value::as_str) != Some("OK") {
        return Err(format!(
            "the generator failed: {status:?}\nmissing builtins: {:?}\n{}",
            engine.interp.missing_builtins, engine.interp.output
        ));
    }
    let parts = fs_geom::with_model(&context, |model| {
        model
            .bodies
            .iter()
            .filter(|body| body.body_type == BodyType::Solid)
            .filter_map(|body| Some(Part { name: body.name()?.to_string(), shape: body.shape.clone() }))
            .collect()
    })
    .ok_or("the context has no geometry")?;
    Ok(Build { parts, output: std::mem::take(&mut engine.interp.output), seconds })
}

/// The generator with the calls of every case not in `only` taken out. Each call is three lines, starting with
/// `if (!runFixtureCase(context, id, "<case>"`.
fn only_cases(generator: &str, only: &[&str]) -> String {
    let mut out = String::new();
    let mut skip = 0;
    for line in generator.lines() {
        if let Some(rest) = line.trim_start().strip_prefix("if (!runFixtureCase(context, id, \"") {
            let name = rest.split('"').next().unwrap_or_default();
            if !only.contains(&name) {
                skip = 3;
            }
        }
        if skip > 0 {
            skip -= 1;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Onshape's parts, from the batch's STEP file.
pub fn read_fixture(batch: &Path) -> Result<Vec<Part>, String> {
    let path = batch.join("onshape.step");
    let parts = occt::read_step(&path.to_string_lossy()).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(parts.into_iter().map(|(name, shape)| Part { name, shape }).collect())
}

/// What the comparison measures about a part.
#[derive(Clone, Debug, PartialEq)]
pub struct Metrics {
    pub volume: f64,
    pub area: f64,
    pub bounding_box: Box3,
    /// Faces by surface type, such as `Plane: 6`.
    pub faces: BTreeMap<String, usize>,
    /// Edges by curve type, apart from seams.
    pub edges: BTreeMap<String, usize>,
    /// Seam edges, where a closed face such as a cylinder's meets itself. Onshape's STEP has none: OpenCascade
    /// adds them as B-splines when it reads the file, so their curve type means nothing.
    pub seams: usize,
    pub vertices: usize,
    pub valid: bool,
}

pub fn metrics(shape: &Shape) -> Result<Metrics, occt::Error> {
    let mut faces = BTreeMap::new();
    for face in shape.sub_shapes(ShapeKind::Face) {
        *faces.entry(format!("{:?}", face.surface_type()?)).or_insert(0) += 1;
    }
    let mut edges = BTreeMap::new();
    let mut seams = 0;
    for edge in shape.sub_shapes(ShapeKind::Edge) {
        if shape.is_seam(&edge) {
            seams += 1;
        } else {
            *edges.entry(format!("{:?}", edge.curve_type()?)).or_insert(0) += 1;
        }
    }
    Ok(Metrics {
        volume: shape.volume()?,
        area: shape.area()?,
        bounding_box: shape.bounding_box(true)?,
        faces,
        edges,
        seams,
        vertices: shape.sub_shapes(ShapeKind::Vertex).len(),
        valid: shape.is_valid(),
    })
}

/// The volume inside one shape but not the other, both ways.
pub fn difference_volume(a: &Shape, b: &Shape) -> Result<f64, occt::Error> {
    let one_way = |x: &Shape, y: &Shape| -> Result<f64, occt::Error> {
        let rest = occt::boolean_fuzzy(
            BooleanKind::Subtraction,
            std::slice::from_ref(x),
            std::slice::from_ref(y),
            false,
            FUZZY,
        )?;
        rest.shape().volume()
    };
    Ok(one_way(a, b)? + one_way(b, a)?)
}

fn box_distance(a: &Box3, b: &Box3) -> f64 {
    let d = |p: f64, q: f64| (p - q).abs();
    [
        d(a.min.x, b.min.x),
        d(a.min.y, b.min.y),
        d(a.min.z, b.min.z),
        d(a.max.x, b.max.x),
        d(a.max.y, b.max.y),
        d(a.max.z, b.max.z),
    ]
    .into_iter()
    .fold(0.0, f64::max)
}

fn relative(a: f64, b: f64) -> f64 {
    if b == 0.0 { a.abs() } else { (a - b).abs() / b.abs() }
}

/// One part's comparison.
#[derive(Clone, Debug)]
pub struct PartResult {
    pub name: String,
    /// Onshape's volume, in m³.
    pub volume: f64,
    pub volume_error: f64,
    pub area_error: f64,
    /// The largest distance between matching bounding box corners, in metres.
    pub box_error: f64,
    /// The volume where the shapes differ, as a fraction of Onshape's volume.
    pub difference: f64,
    /// What didn't match. Empty when the part matches.
    pub problems: Vec<String>,
}

/// Compares the engine's part with Onshape's.
pub fn compare(name: &str, ours: &Shape, theirs: &Shape) -> PartResult {
    let mut result = PartResult {
        name: name.to_string(),
        volume: 0.0,
        volume_error: f64::NAN,
        area_error: f64::NAN,
        box_error: f64::NAN,
        difference: f64::NAN,
        problems: Vec::new(),
    };
    let (ours_m, theirs_m) = match (metrics(ours), metrics(theirs)) {
        (Ok(a), Ok(b)) => (a, b),
        (a, b) => {
            result.problems.push(format!("can't measure: {:?} {:?}", a.err(), b.err()));
            return result;
        }
    };
    result.volume = theirs_m.volume;
    result.volume_error = relative(ours_m.volume, theirs_m.volume);
    result.area_error = relative(ours_m.area, theirs_m.area);
    result.box_error = box_distance(&ours_m.bounding_box, &theirs_m.bounding_box);
    let problems = &mut result.problems;
    if !ours_m.valid {
        problems.push("the engine's shape is invalid".into());
    }
    if result.volume_error > RELATIVE_TOLERANCE {
        problems.push(format!("volume {:.9e} m³, Onshape {:.9e}", ours_m.volume, theirs_m.volume));
    }
    if result.area_error > RELATIVE_TOLERANCE {
        problems.push(format!("area {:.9e} m², Onshape {:.9e}", ours_m.area, theirs_m.area));
    }
    if result.box_error > BOX_TOLERANCE {
        problems.push(format!("bounding box {:?}, Onshape {:?}", ours_m.bounding_box, theirs_m.bounding_box));
    }
    if ours_m.faces != theirs_m.faces {
        problems.push(format!("faces {:?}, Onshape {:?}", ours_m.faces, theirs_m.faces));
    }
    if ours_m.edges != theirs_m.edges {
        problems.push(format!("edges {:?}, Onshape {:?}", ours_m.edges, theirs_m.edges));
    }
    if ours_m.seams != theirs_m.seams {
        problems.push(format!("{} seam edges, Onshape {}", ours_m.seams, theirs_m.seams));
    }
    if ours_m.vertices != theirs_m.vertices {
        problems.push(format!("{} vertices, Onshape {}", ours_m.vertices, theirs_m.vertices));
    }
    match difference_volume(ours, theirs) {
        Ok(volume) => {
            result.difference = volume / theirs_m.volume;
            if result.difference > RELATIVE_TOLERANCE {
                problems.push(format!("the shapes differ by {:.3e} of the volume", result.difference));
            }
        }
        Err(e) => problems.push(format!("difference failed: {e}")),
    }
    result
}

pub struct Report {
    pub parts: Vec<PartResult>,
    /// Parts in Onshape's model that the engine didn't build.
    pub missing: Vec<String>,
    /// Parts the engine built that aren't in Onshape's model.
    pub extra: Vec<String>,
    /// Parts with a name used more than once, in either model.
    pub duplicated: Vec<String>,
    /// Problems reading back the engine's STEP file, if one was written.
    pub step_problems: Vec<String>,
    /// What the generator printed.
    pub output: String,
    pub build_seconds: f64,
}

impl Report {
    /// Every mismatch, one per line.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        out.extend(self.missing.iter().map(|name| format!("{name}: missing from the engine's model")));
        out.extend(self.extra.iter().map(|name| format!("{name}: not in Onshape's model")));
        out.extend(self.duplicated.iter().map(|name| format!("{name}: name used more than once")));
        out.extend(self.step_problems.iter().map(|p| format!("engine STEP: {p}")));
        for part in &self.parts {
            out.extend(part.problems.iter().map(|p| format!("{}: {p}", part.name)));
        }
        out
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let matched = self.parts.iter().filter(|p| p.problems.is_empty()).count();
        writeln!(f, "{matched} of {} parts match Onshape's", self.parts.len())?;
        let worst = |key: fn(&PartResult) -> f64| self.parts.iter().map(key).fold(0.0, f64::max);
        writeln!(
            f,
            "worst errors: volume {:.1e}, area {:.1e}, bounding box {:.1e} m, difference {:.1e} of the volume",
            worst(|p| p.volume_error),
            worst(|p| p.area_error),
            worst(|p| p.box_error),
            worst(|p| p.difference)
        )?;
        for problem in self.problems() {
            writeln!(f, "  {problem}")?;
        }
        Ok(())
    }
}

/// Writes parts to STEP, reads the file back, and checks every part came back with its name, volume and area.
fn write_and_reread(parts: &[Part], path: &Path) -> Result<Vec<String>, String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let shapes: Vec<Shape> = parts.iter().map(|p| p.shape.clone()).collect();
    let names: Vec<String> = parts.iter().map(|p| p.name.clone()).collect();
    let file = path.to_string_lossy();
    occt::write_step(&shapes, &names, &file).map_err(|e| e.to_string())?;
    let reread = occt::read_step(&file).map_err(|e| e.to_string())?;
    let mut problems = Vec::new();
    if reread.len() != parts.len() {
        problems.push(format!("wrote {} parts, read back {}", parts.len(), reread.len()));
    }
    for (part, (name, shape)) in parts.iter().zip(&reread) {
        let (a, b) = (metrics(&part.shape), metrics(shape));
        let same = match (&a, &b) {
            (Ok(a), Ok(b)) => {
                relative(b.volume, a.volume) <= RELATIVE_TOLERANCE && relative(b.area, a.area) <= RELATIVE_TOLERANCE
            }
            _ => false,
        };
        if *name != part.name || !same {
            problems.push(format!("{} came back as {name:?}, matching: {same}", part.name));
        }
    }
    Ok(problems)
}

/// Parts by name; a name can be used more than once.
fn index(parts: &[Part]) -> BTreeMap<String, Vec<&Shape>> {
    let mut by_name: BTreeMap<String, Vec<&Shape>> = BTreeMap::new();
    for part in parts {
        by_name.entry(part.name.clone()).or_default().push(&part.shape);
    }
    by_name
}

/// Builds a batch on the engine and compares it with Onshape's model. With `step`, also writes the engine's
/// model there as STEP.
pub fn check_batch(batch: &Path, step: Option<&Path>) -> Result<Report, String> {
    let built = build(batch)?;
    let fixture = read_fixture(batch)?;
    let step_problems = match step {
        Some(path) => write_and_reread(&built.parts, path)?,
        None => Vec::new(),
    };

    let (ours, theirs) = (index(&built.parts), index(&fixture));
    let mut report = Report {
        parts: Vec::new(),
        missing: theirs.keys().filter(|n| !ours.contains_key(*n)).cloned().collect(),
        extra: ours.keys().filter(|n| !theirs.contains_key(*n)).cloned().collect(),
        duplicated: Vec::new(),
        step_problems,
        output: built.output,
        build_seconds: built.seconds,
    };
    for (name, shapes) in &theirs {
        let Some(our_shapes) = ours.get(name) else { continue };
        if shapes.len() != 1 || our_shapes.len() != 1 {
            report.duplicated.push(name.clone());
            continue;
        }
        report.parts.push(compare(name, our_shapes[0], shapes[0]));
    }
    Ok(report)
}

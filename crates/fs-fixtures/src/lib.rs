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
    // The fixture document is in inches: its default planes are 6 inches across (batch 090 c010).
    let context = engine.eval(module, "newContextWithDefaults(inch)").map_err(exception)?;
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
    // The cases that failed: what one leaves behind and can't delete (a sheet metal part) stays, unnamed.
    let failed: Vec<String> = engine
        .interp
        .output
        .lines()
        .filter_map(|l| l.split_once("failed cases: [").map(|(_, rest)| rest.to_string()))
        .flat_map(|rest| rest.split([',', ' ', ']']).filter(|t| !t.is_empty()).map(str::to_string).collect::<Vec<_>>())
        .collect();
    let parts = fs_geom::with_model(&context, |model| {
        // A closed composite part's bodies are its own; Onshape doesn't export them under their names.
        let consumed: Vec<_> = model
            .bodies
            .iter()
            .filter_map(|body| body.composite.as_ref().filter(|(_, closed)| *closed))
            .flat_map(|(members, _)| members.iter().copied())
            .collect();
        model
            .bodies
            .iter()
            // A sheet metal model's flat pattern isn't a part (nor counted among them).
            .filter(|body| body.body_type == BodyType::Solid && !consumed.contains(&body.tid))
            .filter(|body| !fs_geom::ops::sheet_metal::is_flat_pattern(body))
            // The parts: the named bodies, and what failed cases left (other unnamed solids, such as a form's tool
            // bodies, aren't parts in Onshape).
            .filter(|body| {
                body.name().is_some()
                    || body.info.created_by.get(1).is_some_and(|case| failed.iter().any(|f| **f == **case))
            })
            .enumerate()
            // An unnamed part has Onshape's default name: "Part" and its place among the parts (batch 086 c017's
            // plate, the tenth, is "Part 10").
            .map(|(i, body)| Part {
                name: body.name().map_or_else(|| format!("Part {}", i + 1), str::to_string),
                shape: body.shape.clone(),
            })
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
    // Solids without a name, such as those of a closed composite part, aren't any case's.
    Ok(parts.into_iter().filter(|(name, _)| !name.is_empty()).map(|(name, shape)| Part { name, shape }).collect())
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
    /// Of the B-spline faces, those that are flat (see [`planar_bspline`]).
    pub planar_bsplines: usize,
}

/// Whether a B-spline face is flat: all its poles in one plane. Onshape's STEP may write such a face as a plane (a
/// sweep's sides along a straight path, batch 070 c008, which its own `evSurfaceDefinition` calls B-splines).
fn planar_bspline(face: &Shape) -> bool {
    let Ok(data) = face.face_bspline() else { return false };
    let p = &data.poles;
    let sub = |a: occt::Vec3, b: occt::Vec3| occt::Vec3 { x: a.x - b.x, y: a.y - b.y, z: a.z - b.z };
    let cross = |a: occt::Vec3, b: occt::Vec3| occt::Vec3 {
        x: a.y * b.z - a.z * b.y,
        y: a.z * b.x - a.x * b.z,
        z: a.x * b.y - a.y * b.x,
    };
    let dot = |a: occt::Vec3, b: occt::Vec3| a.x * b.x + a.y * b.y + a.z * b.z;
    let Some(&origin) = p.first() else { return false };
    // The normal of the largest triangle from the first pole.
    let mut normal = occt::Vec3 { x: 0.0, y: 0.0, z: 0.0 };
    for i in 1..p.len() {
        for j in i + 1..p.len() {
            let n = cross(sub(p[i], origin), sub(p[j], origin));
            if dot(n, n) > dot(normal, normal) {
                normal = n;
            }
        }
    }
    let size = dot(normal, normal).sqrt();
    if size < 1e-18 {
        return false;
    }
    p.iter().all(|q| (dot(sub(*q, origin), normal) / size).abs() < 1e-10)
}

pub fn metrics(shape: &Shape) -> Result<Metrics, occt::Error> {
    let mut faces = BTreeMap::new();
    let mut planar_bsplines = 0;
    for face in shape.sub_shapes(ShapeKind::Face) {
        let kind = face.surface_type()?;
        if kind == occt::SurfaceType::BSpline && planar_bspline(&face) {
            planar_bsplines += 1;
        }
        *faces.entry(format!("{kind:?}")).or_insert(0) += 1;
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
        planar_bsplines,
    })
}

/// The volume inside one shape but not the other, both ways.
/// Whether every face of each shape lies on the other, to within `close` metres: points sampled on each face (those
/// inside it) are that close to the other shape.
pub fn surfaces_coincide(a: &Shape, b: &Shape, close: f64) -> bool {
    let one_way = |x: &Shape, y: &Shape| {
        x.sub_shapes(ShapeKind::Face).iter().all(|face| {
            let mut sampled = 0;
            for i in 1..8 {
                for j in 1..8 {
                    let Ok(frame) = face.face_tangent_plane(i as f64 / 8.0, j as f64 / 8.0) else { continue };
                    let Ok(vertex) = occt::vertex(frame.origin) else { continue };
                    if !vertex.distance(face).is_ok_and(|d| d.distance < 1e-9) {
                        continue;
                    }
                    sampled += 1;
                    if !vertex.distance(y).is_ok_and(|d| d.distance < close) {
                        return false;
                    }
                }
            }
            sampled > 0
        })
    };
    one_way(a, b) && one_way(b, a)
}

pub fn difference_volume(a: &Shape, b: &Shape) -> Result<f64, occt::Error> {
    // Booleans between two kernels' B-splines can fail outright; a looser fuzzy value usually gets through.
    let one_way = |x: &Shape, y: &Shape| -> Result<f64, occt::Error> {
        let attempt = |fuzzy: f64| {
            occt::boolean_fuzzy(
                BooleanKind::Subtraction,
                std::slice::from_ref(x),
                std::slice::from_ref(y),
                false,
                fuzzy,
            )
        };
        let rest = attempt(FUZZY).or_else(|_| attempt(FUZZY * 10.0))?;
        rest.shape().volume()
    };
    match (one_way(a, b), one_way(b, a)) {
        (Ok(x), Ok(y)) => Ok(x + y),
        // Or from what they share: V(a) + V(b) - 2 V(a and b).
        (Err(e), _) | (_, Err(e)) => {
            let common = occt::boolean_fuzzy(
                BooleanKind::Intersection,
                std::slice::from_ref(a),
                std::slice::from_ref(b),
                false,
                FUZZY,
            )
            .map_err(|_| e)?;
            Ok((a.volume()? + b.volume()? - 2.0 * common.shape().volume()?).max(0.0))
        }
    }
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

/// How strictly [`compare_with`] checks a part.
#[derive(Clone, Copy, Debug)]
pub struct Strictness {
    /// Volume and area must match to this relative error.
    pub relative: f64,
    /// Compare edge and vertex counts. Onshape's STEP export splits some closed edges, so a batch with probe lines
    /// takes its counts from them instead.
    pub topology: bool,
    /// Where a boolean can't tell the shapes apart, how close their surfaces must lie (metres).
    pub close: f64,
}

/// For batches without probe lines: everything from the STEP file, as tightly as it allows.
pub const STEP_ONLY: Strictness = Strictness { relative: RELATIVE_TOLERANCE, topology: true, close: 1e-7 };

/// For batches with probe lines, which carry Onshape's own volumes, areas and entity counts to 1e-6: the STEP file
/// still checks the shapes (bounding box, face types, and the volume where they differ to 1e-6), but volumes and
/// areas OCCT measures on Onshape's STEP surfaces can be off by 1e-6 (trimmed cylinders), 2e-5 (a B-spline
/// ellipsoid) or 1.3e-4 (a tube swept along an arc, batch 008 c009), so they only catch gross errors here.
pub const WITH_PROBES: Strictness = Strictness { relative: 1e-3, topology: false, close: 1e-7 };

/// Compares the engine's part with Onshape's.
pub fn compare(name: &str, ours: &Shape, theirs: &Shape) -> PartResult {
    compare_with(name, ours, theirs, STEP_ONLY)
}

pub fn compare_with(name: &str, ours: &Shape, theirs: &Shape, strictness: Strictness) -> PartResult {
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
    if result.volume_error > strictness.relative {
        problems.push(format!("volume {:.9e} m³, Onshape {:.9e}", ours_m.volume, theirs_m.volume));
    }
    if result.area_error > strictness.relative {
        problems.push(format!("area {:.9e} m², Onshape {:.9e}", ours_m.area, theirs_m.area));
    }
    // A case compared loosely allows its boxes as far apart as its surfaces (Onshape's twisted sweep along an arc,
    // batch 048: 1.6 µm).
    if result.box_error > BOX_TOLERANCE.max(strictness.close) {
        problems.push(format!("bounding box {:?}, Onshape {:?}", ours_m.bounding_box, theirs_m.bounding_box));
    }
    // Onshape's STEP writes swept faces (extrusions and revolutions of curves) as B-splines, so with probes only
    // the analytic surface types are told apart.
    let face_types = |m: &Metrics| -> BTreeMap<String, usize> {
        if strictness.topology {
            return m.faces.clone();
        }
        let mut out = BTreeMap::new();
        for (kind, count) in &m.faces {
            let kind = match kind.as_str() {
                "Plane" | "Cylinder" | "Cone" | "Sphere" | "Torus" => kind.clone(),
                _ => "FreeForm".to_string(),
            };
            *out.entry(kind).or_insert(0) += count;
        }
        out
    };
    // With probes, the engine's flat B-spline faces may be Onshape's planes or its B-splines: its STEP writes some
    // flat B-splines as planes (batch 070 c008) and keeps others (batch 057 c010).
    let same_faces = if strictness.topology {
        face_types(&ours_m) == face_types(&theirs_m)
    } else {
        let (ours_t, theirs_t) = (face_types(&ours_m), face_types(&theirs_m));
        let count = |m: &BTreeMap<String, usize>, k: &str| m.get(k).copied().unwrap_or(0);
        let as_planes = count(&theirs_t, "Plane") as i64 - count(&ours_t, "Plane") as i64;
        let analytic_same = ["Cylinder", "Cone", "Sphere", "Torus"].iter().all(|k| count(&ours_t, k) == count(&theirs_t, k));
        analytic_same
            && (0..=ours_m.planar_bsplines as i64).contains(&as_planes)
            && count(&ours_t, "FreeForm") as i64 - as_planes == count(&theirs_t, "FreeForm") as i64
    };
    if !same_faces {
        problems.push(format!("faces {:?}, Onshape {:?}", ours_m.faces, theirs_m.faces));
    }
    if strictness.topology && ours_m.edges != theirs_m.edges {
        problems.push(format!("edges {:?}, Onshape {:?}", ours_m.edges, theirs_m.edges));
    }
    if strictness.topology && ours_m.seams != theirs_m.seams {
        problems.push(format!("{} seam edges, Onshape {}", ours_m.seams, theirs_m.seams));
    }
    if strictness.topology && ours_m.vertices != theirs_m.vertices {
        problems.push(format!("{} vertices, Onshape {}", ours_m.vertices, theirs_m.vertices));
    }
    match difference_volume(ours, theirs) {
        Ok(volume) => {
            result.difference = volume / theirs_m.volume;
            // Booleans between two kernels' nearly coincident B-spline faces can give nonsense (a twisted sweep,
            // batch 019); if the surfaces lie on each other, the shapes are the same.
            if result.difference > RELATIVE_TOLERANCE && surfaces_coincide(ours, theirs, strictness.close) {
                result.difference = 0.0;
            }
            if result.difference > RELATIVE_TOLERANCE {
                problems.push(format!("the shapes differ by {:.3e} of the volume", result.difference));
            }
        }
        // A boolean between two kernels' B-splines of the same surface can fail outright (batch 002's ellipsoid);
        // where probes carry Onshape's volume, area and box, that isn't a mismatch.
        Err(e) if strictness.topology => problems.push(format!("difference failed: {e}")),
        Err(_) => {}
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
    /// Probe lines that don't match Onshape's, as "<case>: <what differs>".
    pub probe_problems: Vec<String>,
    /// How many of Onshape's probe lines the engine's matched, and how many there were.
    pub probes_matched: (usize, usize),
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
        out.extend(self.probe_problems.iter().cloned());
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
        if self.probes_matched.1 > 0 {
            writeln!(f, "{} of {} probe lines match Onshape's", self.probes_matched.0, self.probes_matched.1)?;
        }
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
    // A batch of sheets and curves only has no solids to write (batch 039).
    if parts.is_empty() {
        return Ok(Vec::new());
    }
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
            let detail = match (&a, &b) {
                (Ok(a), Ok(b)) => format!(
                    " (volume {:.6} mm³, read back {:.6}; valid: {})",
                    a.volume * 1e9,
                    b.volume * 1e9,
                    part.shape.is_valid()
                ),
                _ => String::new(),
            };
            problems.push(format!("{} came back as {name:?}, matching: {same}{detail}", part.name));
        }
    }
    Ok(problems)
}

/// A probe number must match Onshape's to this absolute error (mm, mm², mm³) plus [`RELATIVE_TOLERANCE`] of its
/// size. Boxes of curved parts can differ by a fraction of a micrometre.
pub const PROBE_ABSOLUTE_TOLERANCE: f64 = 1e-3;

/// Probe lines by case (`"c001"`), and the failed-cases line under `"failed"`.
pub fn probe_lines(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in text.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("probe ") {
            let case = rest.split_whitespace().next().unwrap_or_default();
            out.insert(case.to_string(), line.to_string());
        } else if line.starts_with("batch-") && line.contains(" failed cases: ") {
            out.insert("failed".to_string(), line.to_string());
        }
    }
    out
}

/// Compares two probe lines token by token: numbers to [`PROBE_ABSOLUTE_TOLERANCE`] plus [`RELATIVE_TOLERANCE`]
/// of their size, everything else exactly. Returns the first difference, naming the body it's in.
pub fn compare_probe(ours: &str, theirs: &str) -> Option<String> {
    compare_probe_within(ours, theirs, PROBE_ABSOLUTE_TOLERANCE)
}

/// [`compare_probe`] with another absolute tolerance for numbers.
pub fn compare_probe_within(ours: &str, theirs: &str, absolute: f64) -> Option<String> {
    // Onshape prints vectors as `(1, 2, 3)`: their brackets and commas are tokens of their own.
    let spaced = |line: &str| line.replace('(', " ( ").replace(')', " ) ").replace(',', " , ");
    let (ours, theirs) = (spaced(ours), spaced(theirs));
    let (a, b): (Vec<&str>, Vec<&str>) = (ours.split_whitespace().collect(), theirs.split_whitespace().collect());
    let mut body = "";
    let mut key = "";
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or("<end>"), b.get(i).copied().unwrap_or("<end>"));
        if i >= 2 && b.get(i - 2) == Some(&"name") {
            body = y;
        }
        if b.get(i + 1) == Some(&":") {
            key = y;
        }
        let same = match (x.parse::<f64>(), y.parse::<f64>()) {
            (Ok(p), Ok(q)) => (p - q).abs() <= absolute + RELATIVE_TOLERANCE * p.abs().max(q.abs()),
            _ => x == y,
        };
        if !same {
            let context =
                |tokens: &[&str]| tokens[i.saturating_sub(3).min(tokens.len())..(i + 4).min(tokens.len())].join(" ");
            let place = if body.is_empty() { String::new() } else { format!("{body} ") };
            return Some(format!("{place}{key}: engine `{}`, Onshape `{}`", context(&a), context(&b)));
        }
    }
    None
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
    check_batch_with(batch, step, &[])
}

/// [`check_batch`], comparing the probe lines of some cases with a looser absolute tolerance (in the probes'
/// units, mm): approximations are only promised to within their own tolerance, so two kernels' fits of the same
/// points can differ by that much.
pub fn check_batch_with(batch: &Path, step: Option<&Path>, loose: &[(&str, f64)]) -> Result<Report, String> {
    check_batch_bodies(batch, step, loose, &[])
}

/// [`check_batch_with`], comparing only the bodies of some cases' probe lines: the values they return (Onshape's
/// own B-spline approximations, its faces in its own order) are there to study, not to match.
pub fn check_batch_bodies(
    batch: &Path,
    step: Option<&Path>,
    loose: &[(&str, f64)],
    bodies_only: &[&str],
) -> Result<Report, String> {
    check_batch_unordered(batch, step, loose, bodies_only, &[])
}

/// A probe line with each `faces : [ … ]`, `edges : [ … ]`, `bendLines : [ … ]` and `curves : [ … ]` array's
/// entries sorted by their contents (numbers rounded to 0.01), for cases where Onshape lists them in an order of its
/// own (its internal ids: a loft's sides, batch 060; two wire bodies' edges, not in the bodies' order, batch 061; a
/// PCB's bend lines, batch 071; an extended sheet's curved edges, batch 080), and any other list of faces.
pub fn sort_faces(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    let next = |rest: &str| {
        ["faces : [", "edges : [", "bendLines : [", "curves : ["]
            .iter()
            .filter_map(|m| rest.find(m).map(|i| (i, m.len() - 1)))
            // Any list of faces, whatever its key (batch 083: a floor's), or of edges (batch 087: a region's).
            .chain(rest.find("[ { area :").map(|i| (i, 0)))
            .chain(rest.find("[ { length :").map(|i| (i, 0)))
            .min_by_key(|x| x.0)
    };
    while let Some((start, marker)) = next(rest) {
        let open = start + marker;
        out.push_str(&rest[..=open]);
        let body = &rest[open + 1..];
        // The array's entries, split at its top level.
        let (mut depth, mut end, mut entries, mut from) = (0i32, body.len(), Vec::new(), 0usize);
        for (i, c) in body.char_indices() {
            match c {
                '{' | '[' | '(' => depth += 1,
                '}' | ')' => depth -= 1,
                ']' if depth == 0 => {
                    end = i;
                    break;
                }
                ']' => depth -= 1,
                ',' if depth == 0 => {
                    entries.push(body[from..i].trim().to_string());
                    from = i + 1;
                }
                _ => {}
            }
        }
        if !body[from..end].trim().is_empty() {
            entries.push(body[from..end].trim().to_string());
        }
        let key = |e: &String| -> Vec<String> {
            e.split_whitespace()
                .map(|t| t.parse::<f64>().map_or(t.to_string(), |x| format!("{:020.2}", x + 1e9)))
                .collect()
        };
        entries.sort_by_key(key);
        out.push(' ');
        out.push_str(&entries.join(" , "));
        out.push(' ');
        rest = &body[end..];
    }
    out.push_str(rest);
    out
}

/// [`check_batch_bodies`], with some cases' face and edge lists compared in any order ([`sort_faces`]).
pub fn check_batch_unordered(
    batch: &Path,
    step: Option<&Path>,
    loose: &[(&str, f64)],
    bodies_only: &[&str],
    unordered: &[&str],
) -> Result<Report, String> {
    let built = build(batch)?;
    let fixture = read_fixture(batch)?;
    let step_problems = match step {
        Some(path) => write_and_reread(&built.parts, path)?,
        None => Vec::new(),
    };

    let (ours, theirs) = (index(&built.parts), index(&fixture));
    let probe_file = batch.join("onshape-probes.txt");
    let mut report = Report {
        parts: Vec::new(),
        missing: theirs.keys().filter(|n| !ours.contains_key(*n)).cloned().collect(),
        extra: ours.keys().filter(|n| !theirs.contains_key(*n)).cloned().collect(),
        duplicated: Vec::new(),
        step_problems,
        probe_problems: Vec::new(),
        probes_matched: (0, 0),
        output: built.output,
        build_seconds: built.seconds,
    };
    if probe_file.exists() {
        let theirs = probe_lines(&read(&probe_file)?);
        let ours = probe_lines(&report.output);
        report.probes_matched.1 = theirs.len();
        for (key, line) in &theirs {
            let bodies = |l: &str| -> String {
                if bodies_only.contains(&key.as_str()) {
                    l.split(" , values :").next().unwrap_or(l).to_string()
                } else if unordered.contains(&key.as_str()) {
                    sort_faces(l)
                } else {
                    l.to_string()
                }
            };
            match ours.get(key) {
                None => report.probe_problems.push(format!("{key}: no probe line from the engine")),
                Some(our_line) => match compare_probe_within(
                    &bodies(our_line),
                    &bodies(line),
                    loose.iter().find(|(case, _)| case == key).map_or(PROBE_ABSOLUTE_TOLERANCE, |l| l.1),
                ) {
                    None => report.probes_matched.0 += 1,
                    Some(problem) => report.probe_problems.push(format!("{key} probe: {problem}")),
                },
            }
        }
    }
    let strictness = if probe_file.exists() { WITH_PROBES } else { STEP_ONLY };
    for (name, shapes) in &theirs {
        let Some(our_shapes) = ours.get(name) else { continue };
        if shapes.len() != 1 || our_shapes.len() != 1 {
            report.duplicated.push(name.clone());
            continue;
        }
        // A case compared loosely may have surfaces micrometres apart (Onshape's twisted sweep, batch 019; its conic
        // corner break, batch 020).
        let case = name.split('.').next().unwrap_or(name);
        let strictness =
            if loose.iter().any(|(c, _)| *c == case) { Strictness { close: 5e-6, ..strictness } } else { strictness };
        report.parts.push(compare_with(name, our_shapes[0], shapes[0], strictness));
    }
    Ok(report)
}

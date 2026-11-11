//! The geometry a context holds: bodies, their faces, edges and vertices, and where each one came from.
//!
//! Every body and entity has a transient id, which `@evaluateQuery` hands out as `qTransient` queries, and an
//! [`Info`] saying which feature created it. Operations build new bodies through [`Origins`], which follows OCCT's
//! history so that an entity an operation leaves alone keeps its transient id and one it changes keeps its
//! creator.
//!
//! A model is a list of reference-counted bodies, so cloning it for a feature's rollback snapshot is cheap.

use fs_interp::Value;
use fs_interp::builtins::ContextState;
use occt::{Frame, Shape, ShapeKind};
use std::any::Any;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

pub type FeatureId = Rc<[Rc<str>]>;
pub type Tid = u64;

/// std's `EntityType`, ordered as query results are.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EntityType {
    Body,
    Face,
    Edge,
    Vertex,
}

impl EntityType {
    pub fn from_name(name: &str) -> Option<EntityType> {
        Some(match name {
            "BODY" => EntityType::Body,
            "FACE" => EntityType::Face,
            "EDGE" => EntityType::Edge,
            "VERTEX" => EntityType::Vertex,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            EntityType::Body => "BODY",
            EntityType::Face => "FACE",
            EntityType::Edge => "EDGE",
            EntityType::Vertex => "VERTEX",
        }
    }
}

/// std's `BodyType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyType {
    Solid,
    Sheet,
    Wire,
    Point,
    MateConnector,
    /// A composite part: a body made of other bodies, whose entities are theirs.
    Composite,
}

impl BodyType {
    pub fn name(self) -> &'static str {
        match self {
            BodyType::Composite => "COMPOSITE",
            BodyType::Solid => "SOLID",
            BodyType::Sheet => "SHEET",
            BodyType::Wire => "WIRE",
            BodyType::Point => "POINT",
            BodyType::MateConnector => "MATE_CONNECTOR",
        }
    }

    /// The type of body a shape makes: solid if it has solids, then sheet, wire or point.
    pub fn of(shape: &Shape) -> BodyType {
        if !shape.sub_shapes(ShapeKind::Solid).is_empty() {
            BodyType::Solid
        } else if !shape.sub_shapes(ShapeKind::Face).is_empty() {
            BodyType::Sheet
        } else if !shape.sub_shapes(ShapeKind::Edge).is_empty() {
            BodyType::Wire
        } else {
            BodyType::Point
        }
    }
}

/// Which end of an extrude a cap is at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cap {
    Start,
    End,
}

/// Where an entity came from.
#[derive(Clone, Debug, Default)]
pub struct Info {
    /// The feature (or sub-feature) that created it. `qCreatedBy(id)` matches this id and every id inside it.
    pub created_by: FeatureId,
    /// For sketch entities, the sketch entity's id, such as `"rect1.left"`.
    pub sketch_entity: Option<Rc<str>>,
    pub cap: Option<Cap>,
    /// What it was made from, as transient ids (`qDependency`): an extrude's profile, say.
    pub dependency: Option<Rc<[Tid]>>,
    /// A name an operation gave it, such as the names `@opHole` gives its faces and profile edges (`qOpHoleFace`).
    pub label: Option<Rc<str>>,
    /// The entity whose identity it was made for, such as the sketch point a hole was placed at (`qOpHoleFace`).
    pub identity: Option<Tid>,
    /// For a cylinder or cone, the coordinate system Onshape gives its surface (`evSurfaceDefinition`), which
    /// depends on how it was made.
    pub surface_frame: Option<Frame>,
    /// For the edges and vertices of a sketch's construction curves: construction to `qConstructionFilter`, though
    /// their wire bodies aren't (batches 004 and 029).
    pub construction: bool,
    /// For a straight edge, where Onshape's line is placed, if not at the edge's middle: an extrude's edge swept
    /// from a vertex has it at the far end (a fillet's cylinder takes its origin from it, batch 047).
    pub line_origin: Option<occt::Vec3>,
    /// Other features whose entities it was merged from (an edge where a boolean joined two sheets): `qCreatedBy`
    /// matches them too, as Onshape's does (a sheet metal loft added to a model finds the joint among its loft's
    /// edges, batch 044 c005).
    pub merged_from: Vec<FeatureId>,
    /// For an edge, which way Onshape's runs: a point on it and its direction there (batch 088: an extrude's side
    /// edges run from its end back to its start, a fillet's edges as their older faces' loops run). Edges without
    /// one run as the engine's shape does.
    pub sense: Option<(occt::Vec3, occt::Vec3)>,
    /// For a B-spline edge, flagged periodic though its knots are clamped, which OCCT's curve can't be: std raises
    /// a closed spline's degree so, and Onshape keeps the flag (batch 027 c005).
    pub flagged_periodic: bool,
    /// For a curved face a loft made through arcs: a rational B-spline in Onshape (its STEP export simplifies one
    /// between equal arcs to a cylinder), which it won't replace a face up to, though the engine's between equal
    /// arcs is a cylinder (batches 108, 111). An extension of the sheet keeps it (108 c011).
    pub lofted: bool,
}

impl Info {
    pub fn new(created_by: FeatureId) -> Rc<Info> {
        Rc::new(Info { created_by, sketch_entity: None, cap: None, dependency: None, ..Default::default() })
    }

    /// Info for entities `created_by` made from the entities `dependency`.
    pub fn depending(created_by: FeatureId, dependency: Vec<Tid>) -> Rc<Info> {
        Rc::new(Info {
            created_by,
            sketch_entity: None,
            cap: None,
            dependency: Some(dependency.into()),
            ..Default::default()
        })
    }

    pub fn is_created_by(&self, feature: &[Rc<str>]) -> bool {
        self.created_by.starts_with(feature) || self.merged_from.iter().any(|f| f.starts_with(feature))
    }
}

#[derive(Clone, Debug)]
pub struct Entity {
    pub tid: Tid,
    pub shape: Shape,
    pub info: Rc<Info>,
    /// Where it came from: the transient ids of every entity it was made from, directly or not (tracking queries).
    pub ancestry: Ancestry,
    pub attributes: Rc<Attributes>,
}

/// Attributes set on an entity or body (`@setAttribute`). They stay with it, and its images, as the model changes.
#[derive(Clone, Debug, Default)]
pub struct Attributes {
    pub named: BTreeMap<Rc<str>, Value>,
    /// Legacy unnamed attributes: at most one of each type.
    pub unnamed: Vec<Value>,
}

impl Attributes {
    pub fn is_empty(&self) -> bool {
        self.named.is_empty() && self.unnamed.is_empty()
    }

    /// Adds another entity's attributes, keeping these where both have one of the same name (or type).
    pub fn merge(&mut self, other: &Attributes) {
        for (name, value) in &other.named {
            self.named.entry(name.clone()).or_insert_with(|| value.clone());
        }
        for value in &other.unnamed {
            if !self.unnamed.iter().any(|v| same_type(v, value)) {
                self.unnamed.push(value.clone());
            }
        }
    }
}

/// Whether two values have the same type: the same type tag, or with no tags, the same standard type.
pub fn same_type(a: &Value, b: &Value) -> bool {
    match (a.tag, b.tag) {
        (None, None) => a.standard_type() == b.standard_type(),
        (x, y) => x == y,
    }
}

/// The entities an entity was made from, for tracking queries (`startTracking`).
#[derive(Clone, Debug, Default)]
pub struct Ancestry {
    /// Every entity it was derived from, by modification or generation, directly or through other entities.
    pub all: Rc<[Tid]>,
    /// Only the entities whose identity it carries on: those it is a modified image of (`startTrackingIdentity`).
    pub identity: Rc<[Tid]>,
}

impl Ancestry {
    /// The ancestry of an image of `parent`: `parent` and everything it came from. A modified image carries on its
    /// identity; a generated one doesn't.
    fn child_of(parent: &Entity, modified: bool) -> Ancestry {
        let all = std::iter::once(parent.tid).chain(parent.ancestry.all.iter().copied()).collect();
        let identity = if modified {
            std::iter::once(parent.tid).chain(parent.ancestry.identity.iter().copied()).collect()
        } else {
            Rc::default()
        };
        Ancestry { all, identity }
    }

    /// The ancestry of a copy of the body `parent` (a pattern instance): made from it, without its identity.
    pub fn copy_of_body(parent: &Body) -> Ancestry {
        let all = std::iter::once(parent.tid).chain(parent.ancestry.all.iter().copied()).collect();
        Ancestry { all, identity: Rc::default() }
    }

    /// The ancestry of a body made from `parent`, carrying on its identity.
    fn of_body(parent: &Body) -> Ancestry {
        let all = std::iter::once(parent.tid).chain(parent.ancestry.all.iter().copied()).collect();
        let identity = std::iter::once(parent.tid).chain(parent.ancestry.identity.iter().copied()).collect();
        Ancestry { all, identity }
    }

    fn merge(&mut self, other: &Ancestry) {
        let join = |a: &Rc<[Tid]>, b: &Rc<[Tid]>| -> Rc<[Tid]> {
            let mut out: Vec<Tid> = a.to_vec();
            out.extend(b.iter().filter(|t| !a.contains(t)));
            out.into()
        };
        self.all = join(&self.all, &other.all);
        self.identity = join(&self.identity, &other.identity);
    }
}

/// A body's properties (`@setProperty`), by property type, or `CUSTOM:<id>` for custom properties.
pub type Properties = BTreeMap<Rc<str>, Value>;

#[derive(Clone, Debug)]
pub struct Body {
    pub tid: Tid,
    pub shape: Shape,
    pub body_type: BodyType,
    pub info: Rc<Info>,
    /// Construction geometry: default planes, the origin, construction sketch entities.
    pub construction: bool,
    /// For bodies a sketch made, the sketch's plane.
    pub sketch_plane: Option<Frame>,
    pub properties: Rc<Properties>,
    pub attributes: Rc<Attributes>,
    /// A mate connector's coordinate system.
    pub frame: Option<Frame>,
    /// For a composite part, the transient ids of the bodies in it, and whether it is closed (consumes them).
    pub composite: Option<(Rc<[Tid]>, bool)>,
    /// The bodies it was made from (a piece a split or boolean left of a body), for tracking queries.
    pub ancestry: Ancestry,
    /// For a mate connector, the transient id of the part that owns it (`qMateConnectorsOfParts`).
    pub owner: Option<Tid>,
    /// For a sketch drawn in a sheet metal part's flat pattern, that part's transient id (`qPartsAttachedTo`).
    pub flat_of: Option<Tid>,
    pub faces: Vec<Entity>,
    pub edges: Vec<Entity>,
    pub vertices: Vec<Entity>,
}

impl Body {
    /// Whether it's a sheet metal model's definition sheet: one with a model `SMAttribute`.
    pub fn is_sheet_metal_definition(&self) -> bool {
        self.attributes.unnamed.iter().any(|a| {
            a.tag.is_some_and(|t| &*fs_interp::types::name(t) == "SMAttribute")
                && a.get("objectType").and_then(Value::as_str) == Some("MODEL")
        })
    }

    /// The name set with `@setProperty`, if any.
    pub fn name(&self) -> Option<&str> {
        self.properties.get("NAME").and_then(Value::as_str)
    }

    pub fn entities(&self, kind: EntityType) -> &[Entity] {
        match kind {
            EntityType::Body => &[],
            EntityType::Face => &self.faces,
            EntityType::Edge => &self.edges,
            EntityType::Vertex => &self.vertices,
        }
    }

    pub fn all_entities(&self) -> impl Iterator<Item = &Entity> {
        self.faces.iter().chain(&self.edges).chain(&self.vertices)
    }
}

/// A body, or one of a body's entities, in the model's current state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityRef {
    pub body: usize,
    pub kind: EntityType,
    pub index: usize,
}

impl EntityRef {
    pub fn body(body: usize) -> EntityRef {
        EntityRef { body, kind: EntityType::Body, index: 0 }
    }
}

#[derive(Clone, Default)]
pub struct Model {
    pub bodies: Vec<Rc<Body>>,
    next_tid: Tid,
    /// The id of the last operation that ran (`@lastOperationId`).
    pub last_operation: Option<FeatureId>,
    /// For each face a split made, the split's id, the face's transient id, and whether it is behind the tool (on
    /// the side opposite its normal): `qSplitBy`.
    pub splits: Rc<Vec<(FeatureId, Tid, bool)>>,
    /// Names given to entities (`opNameEntity`), by transient id: `qNamed` finds them and what they become.
    pub names: Rc<Vec<(Rc<str>, Tid)>>,
    /// Whether a sheet metal feature is running, which sees the sheet metal definition bodies that `qEverything`
    /// and `qCreatedBy` otherwise don't (batch 012). Set on every builtin call.
    pub sheet_metal_scope: bool,
    /// Sheet metal forms' placement mate connectors the model has already moved where Onshape leaves them.
    pub placed_forms: Rc<Vec<Tid>>,
}

/// A new body: its shape and what it is. [`Model::add_body`] fills in the entities.
pub struct NewBody {
    pub shape: Shape,
    pub body_type: BodyType,
    pub info: Rc<Info>,
    /// Keep this transient id, for a body an operation changed.
    pub tid: Option<Tid>,
    pub construction: bool,
    pub sketch_plane: Option<Frame>,
    pub properties: Rc<Properties>,
    pub attributes: Rc<Attributes>,
    /// For a mate connector, its coordinate system.
    pub frame: Option<Frame>,
    /// For a composite part, the bodies in it and whether it is closed.
    pub composite: Option<(Rc<[Tid]>, bool)>,
    pub ancestry: Ancestry,
    /// For a mate connector, the part that owns it.
    pub owner: Option<Tid>,
    /// For a sketch in a sheet metal part's flat pattern, that part.
    pub flat_of: Option<Tid>,
}

impl NewBody {
    /// A brand-new body: a fresh transient id, not construction, not from a sketch, no properties.
    pub fn new(shape: Shape, body_type: BodyType, info: Rc<Info>) -> NewBody {
        NewBody {
            shape,
            body_type,
            info,
            tid: None,
            construction: false,
            sketch_plane: None,
            properties: Rc::default(),
            attributes: Rc::default(),
            frame: None,
            composite: None,
            ancestry: Ancestry::default(),
            owner: None,
            flat_of: None,
        }
    }

    /// A body that replaces `old` after an operation: same id, kind, info and properties.
    pub fn replacing(old: &Body, shape: Shape) -> NewBody {
        NewBody {
            shape,
            body_type: old.body_type,
            info: old.info.clone(),
            tid: Some(old.tid),
            construction: old.construction,
            sketch_plane: old.sketch_plane,
            properties: old.properties.clone(),
            attributes: old.attributes.clone(),
            frame: old.frame,
            composite: old.composite.clone(),
            ancestry: Ancestry::of_body(old),
            owner: old.owner,
            flat_of: old.flat_of,
        }
    }
}

impl Model {
    pub fn new_tid(&mut self) -> Tid {
        self.next_tid += 1;
        self.next_tid
    }

    /// Adds a body. Each face, edge and vertex takes its id and info from `special`, then from `origins`, and
    /// otherwise is new and gets `default`.
    pub fn add_body(
        &mut self,
        new: NewBody,
        origins: &Origins,
        default: &Rc<Info>,
        special: &dyn Fn(&Shape) -> Option<Rc<Info>>,
    ) -> usize {
        // Only the topology Onshape would have: no seam or degenerate edges, no vertices on closed edges.
        let mut entities = |kind: ShapeKind| -> Vec<Entity> {
            new.shape
                .visible_sub_shapes(kind)
                .into_iter()
                .map(|shape| {
                    let origin = origins.lookup(&shape);
                    let (tid, info) = match (special(&shape), &origin) {
                        (Some(info), _) => (None, info),
                        (None, Some(o)) => (o.tid, o.info.clone().unwrap_or_else(|| default.clone())),
                        (None, None) => (None, default.clone()),
                    };
                    let (ancestry, attributes) = origin.map(|o| (o.ancestry, o.attributes)).unwrap_or_default();
                    let tid = tid.unwrap_or_else(|| self.new_tid());
                    Entity { tid, shape, info, ancestry, attributes }
                })
                .collect()
        };
        // A composite part's entities are its bodies'.
        let composite = new.body_type == BodyType::Composite;
        let faces = if composite { Vec::new() } else { entities(ShapeKind::Face) };
        let edges = if composite { Vec::new() } else { entities(ShapeKind::Edge) };
        let vertices = if composite { Vec::new() } else { entities(ShapeKind::Vertex) };
        let tid = new.tid.unwrap_or_else(|| self.new_tid());
        self.bodies.push(Rc::new(Body {
            tid,
            shape: new.shape,
            body_type: new.body_type,
            info: new.info,
            construction: new.construction,
            sketch_plane: new.sketch_plane,
            properties: new.properties,
            attributes: new.attributes,
            frame: new.frame,
            composite: new.composite,
            ancestry: new.ancestry,
            owner: new.owner,
            flat_of: new.flat_of,
            faces,
            edges,
            vertices,
        }));
        self.bodies.len() - 1
    }

    /// Changes a body in place (copying it first if a snapshot shares it).
    pub fn update_body(&mut self, index: usize, f: impl FnOnce(&mut Body)) {
        f(Rc::make_mut(&mut self.bodies[index]));
    }

    pub fn remove_bodies(&mut self, indices: &[usize]) {
        let mut index = 0;
        self.bodies.retain(|_| {
            let keep = !indices.contains(&index);
            index += 1;
            keep
        });
    }

    /// The attributes of a body or entity.
    pub fn attributes(&self, r: EntityRef) -> &Rc<Attributes> {
        let body = &self.bodies[r.body];
        match r.kind {
            EntityType::Body => &body.attributes,
            kind => &body.entities(kind)[r.index].attributes,
        }
    }

    /// Changes the attributes of a body or entity.
    /// Records that an entity was also made from these entities (not carrying on their identity).
    pub fn add_ancestors(&mut self, r: EntityRef, from: &[Tid]) {
        self.update_body(r.body, |body| {
            let ancestry = match r.kind {
                EntityType::Body => &mut body.ancestry,
                EntityType::Face => &mut body.faces[r.index].ancestry,
                EntityType::Edge => &mut body.edges[r.index].ancestry,
                EntityType::Vertex => &mut body.vertices[r.index].ancestry,
            };
            let mut all = ancestry.all.to_vec();
            all.extend(from.iter().filter(|t| !ancestry.all.contains(t)));
            ancestry.all = all.into();
        });
    }

    pub fn update_attributes(&mut self, r: EntityRef, f: impl FnOnce(&mut Attributes)) {
        self.update_body(r.body, |body| {
            let attributes = match r.kind {
                EntityType::Body => &mut body.attributes,
                EntityType::Face => &mut body.faces[r.index].attributes,
                EntityType::Edge => &mut body.edges[r.index].attributes,
                EntityType::Vertex => &mut body.vertices[r.index].attributes,
            };
            f(Rc::make_mut(attributes));
        });
    }

    pub fn body(&self, r: EntityRef) -> &Rc<Body> {
        &self.bodies[r.body]
    }

    /// The entity a reference names, or `None` for a body.
    pub fn entity(&self, r: EntityRef) -> Option<&Entity> {
        self.bodies[r.body].entities(r.kind).get(r.index)
    }

    pub fn shape(&self, r: EntityRef) -> &Shape {
        match self.entity(r) {
            Some(entity) => &entity.shape,
            None => &self.bodies[r.body].shape,
        }
    }

    pub fn info(&self, r: EntityRef) -> &Rc<Info> {
        match self.entity(r) {
            Some(entity) => &entity.info,
            None => &self.bodies[r.body].info,
        }
    }

    pub fn tid(&self, r: EntityRef) -> Tid {
        match self.entity(r) {
            Some(entity) => entity.tid,
            None => self.bodies[r.body].tid,
        }
    }

    pub fn find_tid(&self, tid: Tid) -> Option<EntityRef> {
        for (b, body) in self.bodies.iter().enumerate() {
            if body.tid == tid {
                return Some(EntityRef::body(b));
            }
            for kind in [EntityType::Face, EntityType::Edge, EntityType::Vertex] {
                if let Some(index) = body.entities(kind).iter().position(|e| e.tid == tid) {
                    return Some(EntityRef { body: b, kind, index });
                }
            }
        }
        None
    }

    /// Every body and entity, or every one of a type, in model order.
    pub fn everything(&self, kind: Option<EntityType>) -> Vec<EntityRef> {
        let mut out = Vec::new();
        for (b, body) in self.bodies.iter().enumerate() {
            for k in [EntityType::Body, EntityType::Face, EntityType::Edge, EntityType::Vertex] {
                if kind.is_some_and(|kind| kind != k) {
                    continue;
                }
                if k == EntityType::Body {
                    out.push(EntityRef::body(b));
                } else {
                    out.extend((0..body.entities(k).len()).map(|index| EntityRef { body: b, kind: k, index }));
                }
            }
        }
        out
    }

    /// The bodies and entities of `body`, of one type or all.
    pub fn owned_by(&self, body: usize, kind: Option<EntityType>) -> Vec<EntityRef> {
        if let Some((members, _)) = &self.bodies[body].composite {
            return self.composite_members(members).into_iter().flat_map(|b| self.owned_by(b, kind)).collect();
        }
        let mut out = Vec::new();
        for k in [EntityType::Face, EntityType::Edge, EntityType::Vertex] {
            if kind.is_none_or(|kind| kind == k) {
                out.extend((0..self.bodies[body].entities(k).len()).map(|index| EntityRef { body, kind: k, index }));
            }
        }
        out
    }
}

impl Model {
    /// The bodies of a composite part that are still in the model.
    pub fn composite_members(&self, members: &[Tid]) -> Vec<usize> {
        (0..self.bodies.len()).filter(|&b| members.contains(&self.bodies[b].tid)).collect()
    }

    /// The composite parts containing a body.
    pub fn composites_containing(&self, body: usize) -> Vec<usize> {
        let tid = self.bodies[body].tid;
        (0..self.bodies.len())
            .filter(|&c| self.bodies[c].composite.as_ref().is_some_and(|(members, _)| members.contains(&tid)))
            .collect()
    }
}

/// Where the shapes an operation produced came from: input entities that may come through unchanged, and the
/// images of input entities the operation modified or generated.
#[derive(Default)]
pub struct Origins {
    by_hash: HashMap<u64, Vec<(Shape, Origin)>>,
}

/// What a shape an operation produced inherits.
#[derive(Clone)]
pub struct Origin {
    /// The transient id it keeps, if it is an input entity come through unchanged.
    pub tid: Option<Tid>,
    /// The info it takes, or `None` for the operation's own.
    pub info: Option<Rc<Info>>,
    pub ancestry: Ancestry,
    pub attributes: Rc<Attributes>,
}

impl Origins {
    /// An input entity: if it comes through unchanged it keeps its transient id and info.
    pub fn keep(&mut self, entity: &Entity) {
        let origin = Origin {
            tid: Some(entity.tid),
            info: Some(entity.info.clone()),
            ancestry: entity.ancestry.clone(),
            attributes: entity.attributes.clone(),
        };
        self.insert(&entity.shape, origin);
    }

    /// A shape that is an input entity reshaped (an edge moved, a face grown): it keeps the entity's transient id,
    /// info and attributes.
    pub fn same(&mut self, image: &Shape, entity: &Entity) {
        let origin = Origin {
            tid: Some(entity.tid),
            info: Some(entity.info.clone()),
            ancestry: entity.ancestry.clone(),
            attributes: entity.attributes.clone(),
        };
        self.insert(image, origin);
    }

    /// A shape that replaced an input entity: it gets a new transient id but the original's info.
    pub fn inherit(&mut self, image: &Shape, parent: &Entity) {
        let origin = Origin {
            tid: None,
            info: Some(parent.info.clone()),
            ancestry: Ancestry::child_of(parent, true),
            attributes: parent.attributes.clone(),
        };
        self.insert(image, origin);
    }

    /// A copy of an input entity (a pattern instance): new, with the operation's info, tracked back to the entity,
    /// and with its attributes.
    pub fn copy(&mut self, image: &Shape, parent: &Entity) {
        let ancestry = Ancestry::child_of(parent, true);
        self.insert(image, Origin { tid: None, info: None, ancestry, attributes: parent.attributes.clone() });
    }

    /// A shape that carries another context's entity's attributes (merged in from it), but is otherwise new.
    pub fn attributes_from(&mut self, image: &Shape, attributes: &Rc<Attributes>) {
        let origin = Origin { tid: None, info: None, ancestry: Ancestry::default(), attributes: attributes.clone() };
        self.insert(image, origin);
    }

    /// A copy of an input entity with info of its own: new, tracked back to the entity, with its attributes.
    pub fn copy_with_info(&mut self, image: &Shape, parent: &Entity, info: Rc<Info>) {
        let ancestry = Ancestry::child_of(parent, true);
        self.insert(image, Origin { tid: None, info: Some(info), ancestry, attributes: parent.attributes.clone() });
    }

    /// A shape the operation made from an input entity (modified, or generated from it): new, with the operation's
    /// info, but tracked back to the entity.
    pub fn derive(&mut self, image: &Shape, parent: &Entity, modified: bool) {
        let ancestry = Ancestry::child_of(parent, modified);
        self.insert(image, Origin { tid: None, info: None, ancestry, attributes: Rc::default() });
    }

    fn insert(&mut self, shape: &Shape, origin: Origin) {
        let entries = self.by_hash.entry(shape.hash()).or_default();
        match entries.iter_mut().find(|(s, _)| s.is_same(shape)) {
            // An image of several entities (faces merged into one, say) comes from all of them.
            Some((_, existing)) => {
                existing.ancestry.merge(&origin.ancestry);
                if let (Some(info), Some(other)) = (&mut existing.info, &origin.info) {
                    let more: Vec<FeatureId> = std::iter::once(&other.created_by)
                        .chain(&other.merged_from)
                        .filter(|f| **f != info.created_by && !info.merged_from.contains(f))
                        .cloned()
                        .collect();
                    if !more.is_empty() {
                        Rc::make_mut(info).merged_from.extend(more);
                    }
                }
                if !origin.attributes.is_empty() {
                    Rc::make_mut(&mut existing.attributes).merge(&origin.attributes);
                }
            }
            None => entries.push((shape.clone(), origin)),
        }
    }

    /// Origins for an operation on `inputs`: every input entity, every image of one the operation modified (which
    /// inherits its info), and every shape it generated from one (which is new, but tracked back to it).
    pub fn of_operation(op: &occt::Operation, inputs: &[&Body]) -> Origins {
        let mut origins = Origins::default();
        for body in inputs {
            for entity in body.all_entities() {
                origins.keep(entity);
                for image in op.modified(&entity.shape) {
                    origins.inherit(&image, entity);
                }
            }
        }
        origins.inherit_hidden_pieces(op, inputs, &|_| false);
        for body in inputs {
            for entity in body.all_entities() {
                for image in op.generated(&entity.shape) {
                    origins.derive(&image, entity, false);
                }
            }
        }
        origins
    }

    /// The hidden pieces of an input's edges (those OCCT split at a seam vertex, shown as one edge) carry the edge
    /// on into the result as its shown piece does: where an operation keeps or reshapes a hidden piece and the shown
    /// one goes, the edge the result shows is still the input's.
    fn inherit_hidden_pieces(&mut self, op: &occt::Operation, inputs: &[&Body], skip: &dyn Fn(&Shape) -> bool) {
        let mut result_edges: Option<Vec<Shape>> = None;
        for body in inputs {
            let pieces = body.shape.hidden_edge_pieces();
            if pieces.is_empty() {
                continue;
            }
            for entity in &body.edges {
                for (_, hidden) in pieces.iter().filter(|(shown, _)| shown.is_same(&entity.shape)) {
                    let modified = op.modified(hidden);
                    if modified.is_empty() {
                        let edges = result_edges.get_or_insert_with(|| op.shape().sub_shapes(occt::ShapeKind::Edge));
                        if edges.iter().any(|e| e.is_same(hidden)) && !skip(hidden) {
                            self.inherit(hidden, entity);
                        }
                    }
                    for image in modified.iter().filter(|image| !skip(image)) {
                        self.inherit(image, entity);
                    }
                }
            }
        }
    }

    /// As `of_operation`, but an input entity that comes through geometrically unchanged keeps its transient id
    /// and info even when the operation rebuilt it (an offset face's neighbours' far edges, batch 031).
    pub fn of_operation_by_geometry(op: &occt::Operation, inputs: &[&Body]) -> Origins {
        let result = op.shape();
        let mut origins = Origins::default();
        let mut twins: Vec<(Shape, Entity)> = Vec::new();
        for body in inputs {
            for entity in body.all_entities() {
                let Some(kind) = entity.shape.kind() else { continue };
                let candidates = result.sub_shapes(kind);
                if candidates.iter().any(|c| c.is_same(&entity.shape)) {
                    continue;
                }
                if let Some(twin) = candidates.iter().find(|c| same_geometry(c, &entity.shape)) {
                    twins.push((twin.clone(), entity.clone()));
                }
            }
        }
        for (twin, entity) in &twins {
            origins.same(twin, entity);
        }
        let twin_of = |s: &Shape| twins.iter().any(|(t, _)| t.is_same(s));
        for body in inputs {
            for entity in body.all_entities() {
                origins.keep(entity);
                for image in op.modified(&entity.shape) {
                    if !twin_of(&image) {
                        origins.inherit(&image, entity);
                    }
                }
            }
        }
        origins.inherit_hidden_pieces(op, inputs, &twin_of);
        for body in inputs {
            for entity in body.all_entities() {
                for image in op.generated(&entity.shape) {
                    if !twin_of(&image) {
                        origins.derive(&image, entity, false);
                    }
                }
            }
        }
        origins
    }

    /// Origins for an operation that makes new bodies from `inputs`: everything it made from an input entity is new,
    /// with the operation's info, but tracked back to it.
    pub fn derived(op: &occt::Operation, inputs: &[&Entity]) -> Origins {
        let mut origins = Origins::default();
        origins.add_derived(op, inputs);
        origins
    }

    /// Adds `inputs` whose images are new entities, with the operation's info, tracked back to them: what the
    /// operation modified or generated from each, and each that came through unchanged (as a copy, since the input
    /// stays, such as a boolean's kept tools).
    pub fn add_derived(&mut self, op: &occt::Operation, inputs: &[&Entity]) {
        for entity in inputs {
            let modified = op.modified(&entity.shape);
            if modified.is_empty() && !op.is_deleted(&entity.shape) {
                self.derive(&entity.shape, entity, true);
            }
            for image in modified {
                self.derive(&image, entity, true);
            }
            for image in op.generated(&entity.shape) {
                self.derive(&image, entity, false);
            }
        }
    }

    pub fn lookup(&self, shape: &Shape) -> Option<Origin> {
        let entries = self.by_hash.get(&shape.hash())?;
        entries.iter().find(|(s, _)| s.is_same(shape)).map(|(_, origin)| origin.clone())
    }
}

/// A variable set with `@setVariable` or `@setQueryVariable`.
#[derive(Clone, Debug)]
pub struct Variable {
    pub value: Value,
    pub description: Value,
    /// Set with `@setQueryVariable`: only `@getQueryVariable` reads it.
    pub query: bool,
}

/// The state kept in each context: its geometry, its variables, and the feature pattern instances being built (each
/// instance's id and transform, outermost first).
#[derive(Clone, Default)]
pub struct GeomState {
    pub model: Model,
    pub variables: Rc<BTreeMap<Rc<str>, Variable>>,
    pub pattern: Rc<Vec<(FeatureId, occt::Transform)>>,
}

impl ContextState for GeomState {
    fn snapshot(&self) -> Box<dyn Any> {
        Box::new(self.clone())
    }

    fn restore(&mut self, snapshot: Box<dyn Any>) {
        if let Ok(state) = snapshot.downcast::<GeomState>() {
            // Transient ids handed out by the rolled-back feature are never reused.
            let next_tid = self.model.next_tid;
            *self = *state;
            self.model.next_tid = next_tid;
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Whether two shapes of one kind lie in the same place: vertices at one point, edges on one kind of curve through
/// the same ends and middle, faces on one kind of surface with the same area and centroid.
fn same_geometry(a: &Shape, b: &Shape) -> bool {
    use occt::ShapeKind;
    let close = |p: occt::Vec3, q: occt::Vec3| crate::values::norm(crate::values::sub(p, q)) < 1e-9;
    match (a.kind(), b.kind()) {
        (Some(ShapeKind::Vertex), Some(ShapeKind::Vertex)) => match (a.vertex_point(), b.vertex_point()) {
            (Ok(p), Ok(q)) => close(p, q),
            _ => false,
        },
        (Some(ShapeKind::Edge), Some(ShapeKind::Edge)) => {
            if a.curve_type().ok() != b.curve_type().ok() {
                return false;
            }
            let at = |s: &Shape, t: f64| s.edge_tangent(t, true).ok().map(|l| l.origin);
            match (at(a, 0.0), at(a, 0.5), at(a, 1.0), at(b, 0.0), at(b, 0.5), at(b, 1.0)) {
                (Some(a0), Some(am), Some(a1), Some(b0), Some(bm), Some(b1)) => {
                    close(am, bm) && ((close(a0, b0) && close(a1, b1)) || (close(a0, b1) && close(a1, b0)))
                }
                _ => false,
            }
        }
        (Some(ShapeKind::Face), Some(ShapeKind::Face)) => {
            if a.surface_type().ok() != b.surface_type().ok() {
                return false;
            }
            match (a.area(), b.area(), a.centroid(), b.centroid()) {
                (Ok(x), Ok(y), Ok(p), Ok(q)) => (x - y).abs() <= 1e-9 * x.abs().max(1e-12) && close(p, q),
                _ => false,
            }
        }
        _ => false,
    }
}

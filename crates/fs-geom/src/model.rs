//! The geometry a context holds: bodies, their faces, edges and vertices, and where each one came from.
//!
//! Every body and entity has a transient id, which `@evaluateQuery` hands out as `qTransient` queries, and an
//! [`Info`] saying which feature created it. Operations build new bodies through [`Origins`], which follows OCCT's
//! history so that an entity an operation leaves alone keeps its transient id and one it changes keeps its
//! creator.
//!
//! A model is a list of reference-counted bodies, so cloning it for a feature's rollback snapshot is cheap.

use fs_interp::builtins::ContextState;
use occt::{Frame, Shape, ShapeKind};
use std::any::Any;
use std::collections::HashMap;
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
}

impl BodyType {
    pub fn name(self) -> &'static str {
        match self {
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
#[derive(Debug)]
pub struct Info {
    /// The feature (or sub-feature) that created it. `qCreatedBy(id)` matches this id and every id inside it.
    pub created_by: FeatureId,
    /// For sketch entities, the sketch entity's id, such as `"rect1.left"`.
    pub sketch_entity: Option<Rc<str>>,
    pub cap: Option<Cap>,
}

impl Info {
    pub fn new(created_by: FeatureId) -> Rc<Info> {
        Rc::new(Info { created_by, sketch_entity: None, cap: None })
    }

    pub fn is_created_by(&self, feature: &[Rc<str>]) -> bool {
        self.created_by.starts_with(feature)
    }
}

#[derive(Clone, Debug)]
pub struct Entity {
    pub tid: Tid,
    pub shape: Shape,
    pub info: Rc<Info>,
}

#[derive(Debug)]
pub struct Body {
    pub tid: Tid,
    pub shape: Shape,
    pub body_type: BodyType,
    pub info: Rc<Info>,
    /// Construction geometry: default planes, the origin, construction sketch entities.
    pub construction: bool,
    /// For bodies a sketch made, the sketch's plane.
    pub sketch_plane: Option<Frame>,
    pub faces: Vec<Entity>,
    pub edges: Vec<Entity>,
    pub vertices: Vec<Entity>,
}

impl Body {
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
        let mut entities = |kind: ShapeKind| -> Vec<Entity> {
            new.shape
                .sub_shapes(kind)
                .into_iter()
                .map(|shape| {
                    let (tid, info) = match special(&shape) {
                        Some(info) => (None, info),
                        None => origins.lookup(&shape).unwrap_or((None, default.clone())),
                    };
                    let tid = tid.unwrap_or_else(|| self.new_tid());
                    Entity { tid, shape, info }
                })
                .collect()
        };
        let faces = entities(ShapeKind::Face);
        let edges = entities(ShapeKind::Edge);
        let vertices = entities(ShapeKind::Vertex);
        let tid = new.tid.unwrap_or_else(|| self.new_tid());
        self.bodies.push(Rc::new(Body {
            tid,
            shape: new.shape,
            body_type: new.body_type,
            info: new.info,
            construction: new.construction,
            sketch_plane: new.sketch_plane,
            faces,
            edges,
            vertices,
        }));
        self.bodies.len() - 1
    }

    pub fn remove_bodies(&mut self, indices: &[usize]) {
        let mut index = 0;
        self.bodies.retain(|_| {
            let keep = !indices.contains(&index);
            index += 1;
            keep
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
        let mut out = Vec::new();
        for k in [EntityType::Face, EntityType::Edge, EntityType::Vertex] {
            if kind.is_none_or(|kind| kind == k) {
                out.extend((0..self.bodies[body].entities(k).len()).map(|index| EntityRef { body, kind: k, index }));
            }
        }
        out
    }
}

/// Where the shapes an operation produced came from: input entities that may come through unchanged, and the
/// images of input entities the operation modified.
#[derive(Default)]
pub struct Origins {
    by_hash: HashMap<u64, Vec<Origin>>,
}

/// A shape, the transient id it keeps (if it comes through unchanged) and its info.
type Origin = (Shape, Option<Tid>, Rc<Info>);

impl Origins {
    /// An input entity: if it comes through unchanged it keeps its transient id and info.
    pub fn keep(&mut self, entity: &Entity) {
        self.insert(&entity.shape, Some(entity.tid), entity.info.clone());
    }

    /// A shape that replaced an input entity: it gets a new transient id but the original's info.
    pub fn inherit(&mut self, image: &Shape, info: Rc<Info>) {
        self.insert(image, None, info);
    }

    fn insert(&mut self, shape: &Shape, tid: Option<Tid>, info: Rc<Info>) {
        let entries = self.by_hash.entry(shape.hash()).or_default();
        if !entries.iter().any(|(s, _, _)| s.is_same(shape)) {
            entries.push((shape.clone(), tid, info));
        }
    }

    /// Origins for an operation on `inputs`: every input entity, and every image of one the operation modified.
    pub fn of_operation(op: &occt::Operation, inputs: &[&Body]) -> Origins {
        let mut origins = Origins::default();
        for body in inputs {
            for entity in body.all_entities() {
                origins.keep(entity);
                for image in op.modified(&entity.shape) {
                    origins.inherit(&image, entity.info.clone());
                }
            }
        }
        origins
    }

    pub fn lookup(&self, shape: &Shape) -> Option<(Option<Tid>, Rc<Info>)> {
        let entries = self.by_hash.get(&shape.hash())?;
        entries.iter().find(|(s, _, _)| s.is_same(shape)).map(|(_, tid, info)| (*tid, info.clone()))
    }
}

/// The geometry state kept in each context.
#[derive(Clone, Default)]
pub struct GeomState {
    pub model: Model,
}

impl ContextState for GeomState {
    fn snapshot(&self) -> Box<dyn Any> {
        Box::new(self.model.clone())
    }

    fn restore(&mut self, snapshot: Box<dyn Any>) {
        if let Ok(model) = snapshot.downcast::<Model>() {
            // Transient ids handed out by the rolled-back feature are never reused.
            let next_tid = self.model.next_tid;
            self.model = *model;
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

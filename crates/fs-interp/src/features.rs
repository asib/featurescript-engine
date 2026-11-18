//! A Feature Studio's features and their parameters, as a customizer shows them: read from each feature's
//! precondition, where each parameter is a predicate statement with an `annotation { "Name" : ... }` before it.
//!
//! - `isLength(definition.x, BOUNDS)`, `isAngle`, `isInteger` and `isReal`: a number, its bounds and default from the
//!   bound spec (lengths in millimetres, angles in degrees);
//! - `definition.x is boolean` and `definition.x is string`: a check box and a text field;
//! - `definition.x is SomeEnum`: a choice of the enum's values, labelled by their own `Name` annotations;
//! - inside `if (definition.flag)`, `if (!definition.flag)` or `if (definition.choice == SomeEnum.VALUE)`: shown only
//!   when that holds (and in a plain boolean's `else`, when it doesn't).
//!
//! An annotation's `"Default"` overrides a boolean's, a string's or a choice's default. Other parameters (queries,
//! arrays) aren't editable; a feature's definition gets `qNothing()` for a query and leaves the rest out.

use crate::Engine;
use crate::module::ModuleId;
use crate::value::Value;
use fs_lang::ast::{self, BinaryOp, ExprKind, ItemKind, StmtKind, UnaryOp};

/// What kind of value a parameter takes.
#[derive(Clone, Debug, PartialEq)]
pub enum ParameterKind {
    /// A length, in millimetres.
    Length,
    /// An angle, in degrees.
    Angle,
    Integer,
    Real,
    Boolean,
    Text,
    /// One of an enum's values, by name, with each value's label.
    Choice { enum_name: String, options: Vec<(String, String)> },
    /// A query (a selection), which can't be edited here.
    Query,
}

/// A value a parameter can take.
#[derive(Clone, Debug, PartialEq)]
pub enum ParameterValue {
    Number(f64),
    Boolean(bool),
    Text(String),
}

/// A feature parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct FeatureParameter {
    /// The definition field's name.
    pub name: String,
    /// Its annotation's `Name`.
    pub label: String,
    /// Its annotation's `Description`, if any.
    pub description: String,
    pub kind: ParameterKind,
    pub default: ParameterValue,
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// When it applies: another parameter's name and the value it must have.
    pub shown_when: Option<(String, ParameterValue)>,
}

/// FeatureScript that reads a bound spec in the units the customizer uses.
const BOUNDS_HELPER: &str = r#"FeatureScript 3112;
import(path : "onshape/std/geometry.fs", version : "3112.0");

// A bound spec's minimum, default and maximum in `unit`: the range from the entry that has one, the default from the
// entry in `unit` where there is one.
export function customizerBounds(spec is map, unit) returns map
{
    var out = {};
    for (var key, value in spec)
    {
        const scale = (key is ValueWithUnits && unit is ValueWithUnits) ? key / unit : 1;
        if (value is array)
        {
            out.min = value[0] * scale;
            out.max = value[2] * scale;
            if (out["default"] == undefined)
                out["default"] = value[1] * scale;
        }
        if (key is ValueWithUnits && unit is ValueWithUnits && abs(key / unit - 1) < 1e-12)
            out["default"] = value is array ? value[1] : value;
    }
    return out;
}
"#;

impl Engine {
    /// The module's exported features, in source order: consts defined by `defineFeature`.
    pub fn features(&self, module: ModuleId) -> Vec<String> {
        let ast = self.interp.modules.list[module].ast.clone();
        ast.items
            .iter()
            .filter(|item| item.exported)
            .filter_map(|item| match &item.kind {
                ItemKind::Const { name, value, .. } if defined_feature(value).is_some() => Some(name.name.clone()),
                _ => None,
            })
            .collect()
    }

    /// The feature's parameters, in precondition order.
    pub fn feature_parameters(&mut self, module: ModuleId, feature: &str) -> Result<Vec<FeatureParameter>, String> {
        let ast = self.interp.modules.list[module].ast.clone();
        let function = ast
            .items
            .iter()
            .find_map(|item| match &item.kind {
                ItemKind::Const { name, value, .. } if name.name == feature => defined_feature(value),
                _ => None,
            })
            .ok_or_else(|| format!("{feature} isn't a feature (a const defined by defineFeature)"))?;
        let mut out = Vec::new();
        if let Some(precondition) = &function.precondition {
            self.parameters_in(module, precondition, None, &mut out)?;
        }
        Ok(out)
    }

    fn parameters_in(
        &mut self,
        module: ModuleId,
        stmt: &ast::Stmt,
        shown_when: Option<(String, ParameterValue)>,
        out: &mut Vec<FeatureParameter>,
    ) -> Result<(), String> {
        match &stmt.kind {
            StmtKind::Block(block) => {
                for s in &block.stmts {
                    self.parameters_in(module, s, shown_when.clone(), out)?;
                }
            }
            StmtKind::If { cond, then, otherwise } => {
                let condition = self.condition(module, cond);
                self.parameters_in(module, then, condition.clone().map(|(n, v, _)| (n, v)).or(shown_when.clone()), out)?;
                if let Some(otherwise) = otherwise {
                    // The other branch of a plain boolean holds when it's false.
                    let negated = match condition {
                        Some((n, ParameterValue::Boolean(b), true)) => Some((n, ParameterValue::Boolean(!b))),
                        _ => shown_when.clone(),
                    };
                    self.parameters_in(module, otherwise, negated, out)?;
                }
            }
            StmtKind::Expr(expr) => {
                let Some(annotation) = stmt.annotations.first() else { return Ok(()) };
                let annotation = self.evaluate(module, annotation)?;
                let Some(label) = annotation.get("Name").and_then(Value::as_str).map(str::to_string) else { return Ok(()) };
                let description = annotation.get("Description").and_then(Value::as_str).unwrap_or("").to_string();
                let annotated_default = annotation.get("Default").cloned();
                if let Some(parameter) = self.parameter(module, expr, label, description, annotated_default, shown_when)? {
                    out.push(parameter);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// A parameter's predicate, read: the field, its kind, default and bounds.
    fn parameter(
        &mut self,
        module: ModuleId,
        expr: &ast::Expr,
        label: String,
        description: String,
        annotated_default: Option<Value>,
        shown_when: Option<(String, ParameterValue)>,
    ) -> Result<Option<FeatureParameter>, String> {
        let make = |name: String, kind, default, min, max| FeatureParameter {
            name,
            label: label.clone(),
            description: description.clone(),
            kind,
            default,
            min,
            max,
            shown_when: shown_when.clone(),
        };
        match &expr.kind {
            ExprKind::Call { callee, args } => {
                let ExprKind::Name(path) = &callee.kind else { return Ok(None) };
                let kind = match path.name.name.as_str() {
                    "isLength" => ParameterKind::Length,
                    "isAngle" => ParameterKind::Angle,
                    "isInteger" => ParameterKind::Integer,
                    "isReal" => ParameterKind::Real,
                    _ => return Ok(None),
                };
                let (Some(field), Some(bounds)) = (args.first().and_then(definition_field), args.get(1)) else {
                    return Ok(None);
                };
                let spec = self.evaluate(module, bounds)?;
                let unit = match kind {
                    ParameterKind::Length => "millimeter",
                    ParameterKind::Angle => "degree",
                    _ => "undefined",
                };
                let read = self.bounds(spec, unit)?;
                let number = |key: &str| read.get(key).and_then(Value::as_number);
                let default = number("default").ok_or_else(|| format!("{field}'s bounds have no default"))?;
                Ok(Some(make(field, kind, ParameterValue::Number(default), number("min"), number("max"))))
            }
            ExprKind::Is { operand, ty } => {
                let Some(field) = definition_field(operand) else { return Ok(None) };
                let type_name = ty.path.name.name.as_str();
                let text_default = |fallback: &str| {
                    annotated_default.as_ref().and_then(Value::as_str).unwrap_or(fallback).to_string()
                };
                Ok(Some(match type_name {
                    "boolean" => make(
                        field,
                        ParameterKind::Boolean,
                        ParameterValue::Boolean(annotated_default.as_ref().and_then(Value::as_bool).unwrap_or(false)),
                        None,
                        None,
                    ),
                    "string" => make(field, ParameterKind::Text, ParameterValue::Text(text_default("")), None, None),
                    "Query" => make(field, ParameterKind::Query, ParameterValue::Text(String::new()), None, None),
                    other => {
                        let Some(options) = self.enum_options(other)? else { return Ok(None) };
                        let first = options.first().map(|(v, _)| v.clone()).unwrap_or_default();
                        // A choice's default is written as the value itself, `SomeEnum.VALUE`, which evaluates to
                        // its name.
                        let default = annotated_default.as_ref().and_then(Value::as_str).unwrap_or(&first).to_string();
                        make(
                            field,
                            ParameterKind::Choice { enum_name: other.to_string(), options },
                            ParameterValue::Text(default),
                            None,
                            None,
                        )
                    }
                }))
            }
            _ => Ok(None),
        }
    }

    /// An enum's values and their labels, from whichever loaded module declares it.
    fn enum_options(&self, enum_name: &str) -> Result<Option<Vec<(String, String)>>, String> {
        for module in &self.interp.modules.list {
            for item in &module.ast.items {
                let ItemKind::Enum { name, values } = &item.kind else { continue };
                if name.name != enum_name {
                    continue;
                }
                let options = values
                    .iter()
                    .filter(|v| !v.annotations.iter().any(|a| annotation_flag(a, "Hidden")))
                    .map(|v| {
                        let label = v.annotations.iter().find_map(|a| annotation_text(a, "Name"));
                        (v.name.name.clone(), label.unwrap_or_else(|| v.name.name.clone()))
                    })
                    .collect();
                return Ok(Some(options));
            }
        }
        Ok(None)
    }

    /// A simple condition on another parameter: `definition.flag`, `!definition.flag`, or `definition.choice ==`
    /// (or `!=`) `SomeEnum.VALUE`, as the parameter and value that make it hold, and whether it's a plain boolean.
    fn condition(&mut self, module: ModuleId, cond: &ast::Expr) -> Option<(String, ParameterValue, bool)> {
        match &cond.kind {
            ExprKind::Member { .. } => definition_field(cond).map(|f| (f, ParameterValue::Boolean(true), true)),
            ExprKind::Unary { op: UnaryOp::Not, operand } => {
                definition_field(operand).map(|f| (f, ParameterValue::Boolean(false), true))
            }
            ExprKind::Binary { op: BinaryOp::Equal, lhs, rhs } => {
                let field = definition_field(lhs)?;
                let value = self.evaluate(module, rhs).ok()?;
                Some((field, ParameterValue::Text(value.as_str()?.to_string()), false))
            }
            _ => None,
        }
    }

    /// The bound spec read in `unit` (a std unit's name, or `undefined` for numbers).
    fn bounds(&mut self, spec: Value, unit: &str) -> Result<Value, String> {
        let helper = match self.interp.modules.list.iter().position(|m| m.path.ends_with("customizer-bounds.fs")) {
            Some(id) => id,
            None => self.load_source("/customizer-bounds.fs", BOUNDS_HELPER)?,
        };
        let unit = self.eval(helper, unit).map_err(|e| format!("{e:?}"))?;
        self.call(helper, "customizerBounds", vec![spec, unit]).map_err(|e| format!("{e:?}"))
    }

    fn evaluate(&mut self, module: ModuleId, expr: &ast::Expr) -> Result<Value, String> {
        crate::unwrap(self.interp.eval_in_module(module, expr)).map_err(|e| format!("{e:?}"))
    }

    /// The feature's definition for these values (a parameter missing takes its default), as FeatureScript in the
    /// feature's module: lengths in millimetres, angles in degrees, choices by their values' names.
    pub fn feature_definition(
        &mut self,
        module: ModuleId,
        parameters: &[FeatureParameter],
        values: &[(String, ParameterValue)],
    ) -> Result<Value, String> {
        let mut fields = Vec::new();
        for p in parameters {
            let value = values.iter().find(|(n, _)| *n == p.name).map(|(_, v)| v.clone()).unwrap_or(p.default.clone());
            let number = |v: &ParameterValue| match v {
                ParameterValue::Number(n) if n.is_finite() => Ok(*n),
                _ => Err(format!("{} must be a number", p.name)),
            };
            let text = match &p.kind {
                ParameterKind::Length => format!("{} * millimeter", number(&value)?),
                ParameterKind::Angle => format!("{} * degree", number(&value)?),
                ParameterKind::Integer => format!("{}", number(&value)?.round()),
                ParameterKind::Real => format!("{}", number(&value)?),
                ParameterKind::Boolean => match value {
                    ParameterValue::Boolean(b) => b.to_string(),
                    _ => return Err(format!("{} must be true or false", p.name)),
                },
                ParameterKind::Text => match value {
                    ParameterValue::Text(t) => string_literal(&t),
                    _ => return Err(format!("{} must be text", p.name)),
                },
                ParameterKind::Choice { enum_name, options } => match value {
                    ParameterValue::Text(t) if options.iter().any(|(v, _)| *v == t) => format!("{enum_name}.{t}"),
                    _ => return Err(format!("{} must be one of its choices", p.name)),
                },
                ParameterKind::Query => "qNothing()".to_string(),
            };
            fields.push(format!("{} : {text}", string_literal(&p.name)));
        }
        self.eval(module, &format!("{{ {} }}", fields.join(", "))).map_err(|e| format!("{e:?}"))
    }
}

/// The function a `defineFeature(function(...) ...)` call defines.
fn defined_feature(value: &ast::Expr) -> Option<&ast::Function> {
    let ExprKind::Call { callee, args } = &value.kind else { return None };
    let ExprKind::Name(path) = &callee.kind else { return None };
    if path.name.name != "defineFeature" {
        return None;
    }
    match &args.first()?.kind {
        ExprKind::Lambda(function) => Some(function),
        _ => None,
    }
}

/// `definition.x`'s field name.
fn definition_field(expr: &ast::Expr) -> Option<String> {
    let ExprKind::Member { target, name, .. } = &expr.kind else { return None };
    let ExprKind::Name(path) = &target.kind else { return None };
    (path.name.name == "definition").then(|| name.name.clone())
}

/// An annotation map literal's entry for `key`, written as a bare name or a string.
fn annotation_entry<'a>(annotation: &'a ast::Expr, key: &str) -> Option<&'a ast::Expr> {
    let ExprKind::Map(entries) = &annotation.kind else { return None };
    entries.iter().find_map(|entry| {
        let matches = match &entry.key {
            ast::MapKey::Name(name) => name.name == key,
            ast::MapKey::Expr(e) => matches!(&e.kind, ExprKind::String(k) if k == key),
        };
        matches.then_some(&entry.value)
    })
}

/// An annotation map literal's string entry.
fn annotation_text(annotation: &ast::Expr, key: &str) -> Option<String> {
    match &annotation_entry(annotation, key)?.kind {
        ExprKind::String(v) => Some(v.clone()),
        _ => None,
    }
}

/// Whether an annotation map literal sets `key` to true.
fn annotation_flag(annotation: &ast::Expr, key: &str) -> bool {
    annotation_entry(annotation, key).is_some_and(|v| matches!(v.kind, ExprKind::Bool(true)))
}

/// A FeatureScript string literal.
fn string_literal(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

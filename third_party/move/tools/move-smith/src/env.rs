// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Manages the various information during generation

use crate::{
    config::Config,
    names::{Identifier, IdentifierKind as IDKind, IdentifierPool, Scope},
    types::{Type, TypePool},
};
use arbitrary::Unstructured;
use log::trace;
use std::collections::{BTreeMap, BTreeSet};

/// The meta store for all the information during generation
#[derive(Debug)]
pub struct Env {
    pub config: Config,
    pub id_pool: IdentifierPool,
    pub type_pool: TypePool,

    pub live_vars: LiveVarPool,

    /// For controlling the depth of the generated expressions/types
    max_expr_depth: usize,
    max_expr_depth_history: Vec<usize>,

    /// The current depth of the generated expressions
    expr_depth: usize,
    expr_depth_history: Vec<usize>,

    /// The current depth of the generated types
    type_depth: usize,
    type_depth_history: Vec<usize>,

    /// Timeout
    start_time: std::time::Instant,
    timeout: std::time::Duration,

    /// Inline function counter
    inline_func_counter: usize,

    /// Number of fields that has type of another struct
    struct_type_field_counter: usize,
}

/// NOTE: This is unused for now to avoid the situation where the fuzzer cannot
/// find any expression for a type. Now everything is copy+drop.
///
/// Keep track of if a variable is still alive within a certain scope
///
/// If a variable might be dead, it is dead.
/// e.g. if a variable is consumer in one branch of an ITE, it is considered used.
#[derive(Debug, Default)]
pub struct LiveVarPool {
    scopes: BTreeMap<Scope, BTreeSet<Identifier>>,
}

impl LiveVarPool {
    /// Create am empty LiveVarPool
    pub fn new() -> Self {
        Self {
            scopes: BTreeMap::new(),
        }
    }

    /// Check if an identifier is still alive in any parent scope
    pub fn is_live(&self, scope: &Scope, id: &Identifier) -> bool {
        scope
            .ancestors()
            .iter()
            .rev()
            .any(|s| self.is_live_curr(s, id))
    }

    /// Check if an identifier is still alive strictly in the given scope
    pub fn is_live_curr(&self, scope: &Scope, id: &Identifier) -> bool {
        self.scopes.get(scope).map_or(false, |s| s.contains(id))
    }

    /// Filter out non-live identifiers
    pub fn filter_live_vars(&self, scope: &Scope, ids: Vec<Identifier>) -> Vec<Identifier> {
        ids.into_iter()
            .filter(|id| self.is_live(scope, id))
            .collect()
    }

    /// Mark an identifier as alive in the given scope and all its parent scopes
    pub fn mark_alive(&mut self, scope: &Scope, id: &Identifier) {
        trace!("Marking {:?} as alive in {:?}", id, scope);
        let live_vars = self.scopes.entry(scope.clone()).or_default();
        live_vars.insert(id.clone());
    }

    /// Mark an identifier as dead
    pub fn mark_moved(&mut self, scope: &Scope, id: &Identifier) {
        trace!("Marking {:?} as moved in {:?}", id, scope);
        // The varibale is consumed at the given scope, but might be assigned
        // (marked alive) at an earlier scope, so we need to check back.
        scope.ancestors().iter().for_each(|s| {
            if let Some(live_vars) = self.scopes.get_mut(s) {
                live_vars.remove(id);
            }
        });
    }
}

impl Env {
    /// Create a new environment with the given configuration
    pub fn new(config: &Config) -> Self {
        Self {
            config: config.clone(),
            id_pool: IdentifierPool::new(),
            type_pool: TypePool::new(),

            live_vars: LiveVarPool::new(),

            max_expr_depth: config.max_expr_depth,
            max_expr_depth_history: vec![],
            expr_depth: 0,
            expr_depth_history: vec![],
            type_depth: 0,
            type_depth_history: vec![],

            start_time: std::time::Instant::now(),
            timeout: std::time::Duration::from_secs(config.generation_timeout_sec as u64),

            inline_func_counter: 0,
            struct_type_field_counter: 0,
        }
    }

    /// Check if the current generation has reached the timeout
    #[inline]
    pub fn check_timeout(&self) -> bool {
        self.start_time.elapsed() > self.timeout
    }

    /// Return a list of identifiers fileterd by the given type and scope
    /// `typ` should be the desired Move type
    /// `ident_type` should be the desired identifier type (e.g. var, func)
    /// `scope` should be the desired scope
    pub fn get_identifiers(
        &self,
        typ: Option<&Type>,
        ident_kind: Option<IDKind>,
        scope: Option<&Scope>,
    ) -> Vec<Identifier> {
        let mut ids = self.get_identifiers_all(typ, ident_kind, scope);
        ids.retain(|id| !matches!(self.type_pool.get_type(id), Some(Type::Vector(_))));
        ids
    }

    pub fn get_vector_identifiers(&self, typ: Option<&Type>, scope: &Scope) -> Vec<Identifier> {
        let mut ids = self.get_identifiers_all(typ, Some(IDKind::Var), Some(scope));
        ids.retain(|id| matches!(self.type_pool.get_type(id), Some(Type::Vector(_))));
        ids
    }

    fn get_identifiers_all(
        &self,
        typ: Option<&Type>,
        ident_kind: Option<IDKind>,
        scope: Option<&Scope>,
    ) -> Vec<Identifier> {
        trace!(
            "Getting identifiers with constraints: typ ({:?}), kind ({:?}), scope ({:?})",
            typ,
            ident_kind,
            scope,
        );

        // Filter based on the IDKind
        let all_ident = match ident_kind {
            Some(ref t) => self.id_pool.get_identifiers_of_ident_kind(t.clone()),
            None => self.id_pool.get_all_identifiers(),
        };
        trace!(
            "After filtering identifier kind {:?}, {} identifiers remined",
            ident_kind,
            all_ident.len()
        );

        // Filter based on Scope
        let ident_in_scope = match scope {
            Some(s) => self.id_pool.filter_identifier_in_scope(&all_ident, s),
            None => all_ident,
        };
        trace!(
            "After filtering scope {:?}, {} identifiers remined",
            scope,
            ident_in_scope.len()
        );

        // Filter based on Type
        let type_matched = match typ {
            Some(t) => self
                .type_pool
                .filter_identifier_with_type(t, ident_in_scope),
            None => ident_in_scope,
        };
        trace!(
            "After filtering type {:?}, {} identifiers remined",
            typ,
            type_matched.len()
        );

        // Filter out the identifiers that do not have a type
        // i.e. the one just declared but the RHS of assign is not finished yet
        type_matched
            .into_iter()
            .filter(|id: &Identifier| self.type_pool.get_type(id).is_some())
            .collect()
    }

    /// Return the list of live variables of type `typ` in the given scope
    pub fn live_variables(&self, scope: &Scope, typ: Option<&Type>) -> Vec<Identifier> {
        let ids = self.get_identifiers(typ, Some(IDKind::Var), Some(scope));
        self.live_vars.filter_live_vars(scope, ids)
    }

    /// Return whether the current expression depth has reached the limit
    pub fn reached_expr_depth_limit(&self) -> bool {
        self.expr_depth >= self.max_expr_depth
    }

    /// Return whether the current expression depth will reach the limit
    /// with `inc` more layers
    pub fn will_reached_expr_depth_limit(&self, inc: usize) -> bool {
        self.expr_depth + inc >= self.max_expr_depth
    }

    /// Return the current expression depth
    #[inline]
    pub fn curr_expr_depth(&self) -> usize {
        self.expr_depth
    }

    /// Set a temporary maximum expression depth.
    /// Old value will be recorded and can be restored by `reset_max_expr_depth`
    pub fn set_max_expr_depth(&mut self, max_expr_depth: usize) {
        self.max_expr_depth_history.push(self.max_expr_depth);
        self.max_expr_depth = max_expr_depth;
    }

    /// Restore the maximum expression depth to the previous value.
    /// Should always be called with `set_max_expr_depth` in pair
    pub fn reset_max_expr_depth(&mut self) {
        self.max_expr_depth = self.max_expr_depth_history.pop().unwrap();
    }

    /// Randomly choose a number of depth to increase the expression depth.
    /// This allows us to end early in some cases.
    #[inline]
    pub fn increase_expr_depth(&mut self, u: &mut Unstructured) {
        let inc = u.choose(&[1, 2, 3]).unwrap();
        self.expr_depth += *inc;
        self.expr_depth_history.push(*inc);
        trace!("Increment expr depth by {} to: {}", *inc, self.expr_depth,);
    }

    /// Decrease the expression depth by the last increased amount.
    /// This should be called after `increase_expr_depth` and
    /// they should always be in pairs.
    #[inline]
    pub fn decrease_expr_depth(&mut self) {
        let dec = self.expr_depth_history.pop().unwrap();
        self.expr_depth -= dec;
        trace!("Decrement expr depth to: {}", self.expr_depth);
    }

    /// Randomly choose a number of depth to increase the type depth.
    /// This allows us to end early in some cases.
    #[inline]
    pub fn increase_type_depth(&mut self, u: &mut Unstructured) {
        let inc = u.choose(&[1, 2, 3]).unwrap();
        self.type_depth += *inc;
        self.type_depth_history.push(*inc);
        trace!("Increment type depth by {} to: {}", *inc, self.type_depth,);
    }

    /// Decrease the type depth by the last increased amount.
    /// This should be called after `increase_type_depth` and
    /// they should always be in pairs.
    #[inline]
    pub fn decrease_type_depth(&mut self) {
        let dec = self.type_depth_history.pop().unwrap();
        self.type_depth -= dec;
        trace!("Decrement type depth to: {}", self.type_depth);
    }

    /// Check if the current type depth has reached the limit
    #[inline]
    pub fn reached_type_depth_limit(&self) -> bool {
        self.type_depth >= self.max_expr_depth
    }

    #[inline]
    pub fn inc_inline_func_counter(&mut self) {
        self.inline_func_counter += 1;
    }

    #[inline]
    pub fn reached_inline_function_limit(&self) -> bool {
        self.inline_func_counter >= self.config.max_num_inline_funcs
    }

    #[inline]
    pub fn inc_struct_type_field_counter(&mut self) {
        self.struct_type_field_counter += 1;
    }

    #[inline]
    pub fn reached_struct_type_field_limit(&self) -> bool {
        self.struct_type_field_counter >= self.config.max_num_fields_of_struct_type
    }
}

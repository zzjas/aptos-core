// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use arbitrary::Arbitrary;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Identifier(pub String);

impl Identifier {
    pub fn to_scope(&self) -> Scope {
        Scope(Some(self.0.clone()))
    }
}

// pub type Scope = Option<String>;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Scope(pub Option<String>);

pub const ROOT_SCOPE: Scope = Scope(None);

pub fn merge_scopes(parent: &Scope, child: &Scope) -> Scope {
    Scope(match (&parent.0, &child.0) {
        (Some(p), Some(c)) => Some(format!("{}::{}", p, c)),
        (Some(p), None) => Some(p.clone()),
        (None, Some(c)) => Some(c.clone()),
        (None, None) => None,
    })
}

pub fn is_in_scope(child: &Scope, parent: &Scope) -> bool {
    match (&child.0, &parent.0) {
        (Some(c), Some(p)) => c == p || c.starts_with(&format!("{}::", p)),
        (Some(_), None) => true,
        (None, Some(_)) => false,
        (None, None) => true,
    }
}

#[derive(Debug)]
pub struct IdentifierPool {
    vars: Vec<Identifier>,
    structs: Vec<Identifier>,
    functions: Vec<Identifier>,
    modules: Vec<Identifier>,
    scripts: Vec<Identifier>,
    constants: Vec<Identifier>,

    scopes: HashMap<Identifier, Scope>,
}

#[derive(Debug, Clone, Arbitrary)]
pub enum IdentifierType {
    Var,
    Struct,
    Function,
    Module,
    Script,
    Constant,
}

impl Default for IdentifierPool {
    fn default() -> Self {
        Self::new()
    }
}

impl IdentifierPool {
    pub fn new() -> Self {
        Self {
            vars: Vec::new(),
            structs: Vec::new(),
            functions: Vec::new(),
            modules: Vec::new(),
            scripts: Vec::new(),
            constants: Vec::new(),
            scopes: HashMap::new(),
        }
    }

    pub fn next_identifier(&mut self, typ: IdentifierType, scope: &Scope) -> (Identifier, Scope) {
        let cnt = self.identifier_count(&typ);
        let name = self.construct_name(&typ, cnt);
        self.insert_new_identifier(&typ, Identifier(name.clone()));
        self.scopes.insert(Identifier(name.clone()), scope.clone());
        let child_scope = Scope(Some(name.clone()));
        let scope = merge_scopes(scope, &child_scope);
        (Identifier(name), scope)
    }

    pub fn get_parent_scope_of(&self, id: &Identifier) -> Option<Scope> {
        self.scopes.get(id).cloned()
    }

    pub fn get_scope_for_children(&self, id: &Identifier) -> Scope {
        match self.scopes.get(id) {
            Some(scope) => merge_scopes(scope, &id.to_scope()),
            None => id.to_scope(),
        }
    }

    pub fn flatten_access(&self, id: &Identifier) -> Option<Identifier> {
        match self.get_scope_for_children(id) {
            Scope(Some(scope)) => Some(Identifier(scope)),
            Scope(None) => None,
        }
    }

    pub fn filter_identifier_in_scope(
        &self,
        identifiers: &Vec<Identifier>,
        parent_scope: &Scope,
    ) -> Vec<Identifier> {
        let mut in_scope = Vec::new();
        for id in identifiers {
            let id_scope = self.scopes.get(id).unwrap_or(&ROOT_SCOPE);
            if is_in_scope(id_scope, parent_scope) {
                in_scope.push(id.clone());
            }
        }
        in_scope
    }

    pub fn get_all_identifiers(&self) -> Vec<Identifier> {
        self.scopes.keys().cloned().collect()
    }

    pub fn get_identifiers_of_ident_type(&self, typ: IdentifierType) -> Vec<Identifier> {
        self._get_identifiers_of_ident_type(typ).clone()
    }

    fn _get_identifiers_of_ident_type(&self, typ: IdentifierType) -> &Vec<Identifier> {
        match typ {
            IdentifierType::Var => &self.vars,
            IdentifierType::Struct => &self.structs,
            IdentifierType::Function => &self.functions,
            IdentifierType::Module => &self.modules,
            IdentifierType::Script => &self.scripts,
            IdentifierType::Constant => &self.constants,
        }
    }

    fn insert_new_identifier(&mut self, typ: &IdentifierType, name: Identifier) {
        match typ {
            IdentifierType::Var => self.vars.push(name),
            IdentifierType::Struct => self.structs.push(name),
            IdentifierType::Function => self.functions.push(name),
            IdentifierType::Module => self.modules.push(name),
            IdentifierType::Script => self.scripts.push(name),
            IdentifierType::Constant => self.constants.push(name),
        }
    }

    fn identifier_count(&self, typ: &IdentifierType) -> usize {
        match typ {
            IdentifierType::Var => self.vars.len(),
            IdentifierType::Struct => self.structs.len(),
            IdentifierType::Function => self.functions.len(),
            IdentifierType::Module => self.modules.len(),
            IdentifierType::Script => self.scripts.len(),
            IdentifierType::Constant => self.constants.len(),
        }
    }

    fn construct_name(&self, typ: &IdentifierType, idx: usize) -> String {
        let type_prefix = match typ {
            IdentifierType::Var => "var",
            IdentifierType::Struct => "Struct",
            IdentifierType::Function => "function",
            IdentifierType::Module => "Module",
            IdentifierType::Script => "Script",
            IdentifierType::Constant => "CONST",
        };
        format!("{}{}", type_prefix, idx)
    }
}

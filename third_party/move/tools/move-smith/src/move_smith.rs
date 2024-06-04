// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! This is the core generation logic for MoveSmith.
//! Each MoveSmith instance can generates a single Move program consisting of
//! multiple modules and a script.
//! Each generated unit should be runnable as a transactional test.
//! The generation is deterministic. Using the same input Unstructured byte
//! sequence would lead to the same output.
//!
//! The generation for modules is divided into two phases:
//! 1. Generate the skeleton of several elements so that they can be referenced later.
//!     - Generate module names
//!     - Generate struct names and abilities
//!     - Generate function names and signatures
//! 2. Fill in the details of the generated elements.
//!     - Fill in struct fields
//!     - Fill in function bodies

use crate::{
    ast::*,
    config::Config,
    names::{is_in_scope, Identifier, IdentifierPool, IdentifierType, Scope, ROOT_SCOPE},
    types::{Type, TypePool},
};
use arbitrary::{Arbitrary, Result, Unstructured};
use num_bigint::BigUint;
use std::cell::RefCell;

/// Keeps track of the generation state.
pub struct MoveSmith {
    pub config: Config,

    // The output code
    pub modules: Vec<RefCell<Module>>,
    pub script: Option<Script>,

    // Skeleton Information
    function_signatures: Vec<FunctionSignature>,

    // Bookkeeping
    pub id_pool: RefCell<IdentifierPool>,
    pub type_pool: RefCell<TypePool>,
}

impl Default for MoveSmith {
    /// Create a new MoveSmith instance with default configuration.
    fn default() -> Self {
        Self::new(Config::default())
    }
}

impl MoveSmith {
    /// Create a new MoveSmith instance with the given configuration.
    pub fn new(config: Config) -> Self {
        Self {
            config,
            modules: Vec::new(),
            script: None,
            function_signatures: Vec::new(),
            id_pool: RefCell::new(IdentifierPool::new()),
            type_pool: RefCell::new(TypePool::new()),
        }
    }

    /// Get the generated compile unit.
    pub fn get_compile_unit(&self) -> CompileUnit {
        let modules = self
            .modules
            .iter()
            .map(|m| m.borrow().clone())
            .collect::<Vec<Module>>();
        CompileUnit {
            modules,
            scripts: match &self.script {
                Some(s) => vec![s.clone()],
                None => Vec::new(),
            },
        }
    }

    /// Generate a Move program consisting of multiple modules and a script.
    /// Consumes the given Unstructured instance to guide the generation.
    ///
    /// Script is generated after all modules are generated so that the script can call functions.
    pub fn generate(&mut self, u: &mut Unstructured) -> Result<()> {
        let num_modules = u.int_in_range(1..=self.config.max_num_modules)?;

        for _ in 0..num_modules {
            self.modules
                .push(RefCell::new(self.generate_module_skeleton(u)?));
        }

        for m in self.modules.iter() {
            self.fill_module(u, m)?;
        }

        self.script = Some(self.generate_script(u)?);

        Ok(())
    }

    /// Generate a script that calls functions from the generated modules.
    fn generate_script(&self, u: &mut Unstructured) -> Result<Script> {
        let mut script = Script { main: Vec::new() };

        let mut all_funcs: Vec<RefCell<Function>> = Vec::new();
        for m in self.modules.iter() {
            for f in m.borrow().functions.iter() {
                all_funcs.push(f.clone());
            }
        }

        for _ in 0..u.int_in_range(1..=self.config.max_num_calls_in_script)? {
            let func = u.choose(&all_funcs)?;
            let mut call =
                self.generate_call_to_function(u, &ROOT_SCOPE, &func.borrow().signature, false)?;
            call.name = self.id_pool.borrow().flatten_access(&call.name).unwrap();
            script.main.push(call);
        }

        Ok(script)
    }

    /// Generate a module skeleton with only struct and function skeletions.
    fn generate_module_skeleton(&self, u: &mut Unstructured) -> Result<Module> {
        let hardcoded_address = Scope(Some("0xCAFE".to_string()));
        let (name, scope) = self.get_next_identifier(IdentifierType::Module, &hardcoded_address);

        // Struct names
        let mut structs = Vec::new();
        for _ in 0..u.int_in_range(1..=self.config.max_num_structs_in_module)? {
            structs.push(RefCell::new(self.generate_struct_skeleton(u, &scope)?));
        }

        // Function signatures
        let mut functions = Vec::new();
        for _ in 0..u.int_in_range(1..=self.config.max_num_functions_in_module)? {
            functions.push(RefCell::new(self.generate_function_skeleton(u, &scope)?));
        }

        Ok(Module {
            name,
            functions,
            structs,
        })
    }

    /// Fill in the skeletons
    fn fill_module(&self, u: &mut Unstructured, module: &RefCell<Module>) -> Result<()> {
        let scope = self
            .id_pool
            .borrow()
            .get_scope_for_children(&module.borrow().name);
        // Struct fields
        for s in module.borrow().structs.iter() {
            self.fill_struct(u, s, &scope)?;
        }

        // Function bodies
        for f in module.borrow().functions.iter() {
            self.fill_function(u, f)?;
        }

        Ok(())
    }

    // Generate a struct skeleton with name and random abilities.
    fn generate_struct_skeleton(
        &self,
        u: &mut Unstructured,
        parent_scope: &Scope,
    ) -> Result<StructDefinition> {
        let (name, _) = self.get_next_identifier(IdentifierType::Struct, parent_scope);

        let mut ability_choices = vec![Ability::Copy, Ability::Drop, Ability::Store, Ability::Key];
        let mut abilities = Vec::new();
        for _ in 0..u.int_in_range(0..=3)? {
            let idx = u.int_in_range(0..=(ability_choices.len() - 1))?;
            abilities.push(ability_choices.remove(idx));
        }

        self.type_pool
            .borrow_mut()
            .register_type(Type::Struct(name.clone()));
        Ok(StructDefinition {
            name,
            abilities,
            fields: Vec::new(),
        })
    }

    /// Fill in the struct fields with random types.
    fn fill_struct(
        &self,
        u: &mut Unstructured,
        st: &RefCell<StructDefinition>,
        parent_scope: &Scope,
    ) -> Result<()> {
        let struct_scope = st.borrow().name.to_scope();
        for _ in 0..u.int_in_range(0..=self.config.max_num_fields_in_struct)? {
            let (name, _) = self.get_next_identifier(IdentifierType::Var, &struct_scope);

            let typ = loop {
                match u.int_in_range(0..=2)? {
                    // More chance to use basic types than struct types
                    0 | 1 => break self.generate_basic_type(u)?,
                    2 => {
                        let candidates = self.get_usable_struct_type(
                            st.borrow().abilities.clone(),
                            parent_scope,
                            &st.borrow().name,
                        );
                        if !candidates.is_empty() {
                            break Type::Struct(u.choose(&candidates)?.name.clone());
                        }
                    },
                    _ => panic!("Invalid type"),
                }
            };
            // Keeps track of the type of the field
            self.type_pool.borrow_mut().insert_mapping(&name, &typ);
            st.borrow_mut().fields.push((name, typ));
        }
        Ok(())
    }

    /// Return all struct definitions that:
    /// * with in the same module (TODO: allow cross module reference)
    /// * have the desired abilities
    /// * if key is in desired abilities, the struct must have store ability
    /// * does not create loop in the struct hierarchy (TODO: fix the check)
    fn get_usable_struct_type(
        &self,
        desired: Vec<Ability>,
        scope: &Scope,
        parent_struct_id: &Identifier,
    ) -> Vec<StructDefinition> {
        let ids = self.get_filtered_identifiers(None, Some(IdentifierType::Struct), Some(scope));
        ids.iter()
            .filter_map(|s| {
                let struct_def = self.get_struct_definition_with_identifier(s).unwrap();
                if !desired.iter().all(|a| struct_def.abilities.contains(a)) {
                    return None;
                }
                if desired.contains(&Ability::Key)
                    && !struct_def.abilities.contains(&Ability::Store)
                {
                    return None;
                }
                if self.check_struct_reachable(&struct_def.name, parent_struct_id) {
                    return None;
                }
                Some(struct_def)
            })
            .collect()
    }

    /// Check if the struct is reachable from another struct.
    fn check_struct_reachable(&self, source: &Identifier, sink: &Identifier) -> bool {
        if source == sink {
            return true;
        }
        let source_struct = self.get_struct_definition_with_identifier(source).unwrap();
        for (_, typ) in source_struct.fields.iter() {
            let name = match typ {
                Type::Struct(id) => id,
                _ => continue,
            };
            if name == sink {
                return true;
            }
            if self.check_struct_reachable(name, sink) {
                return true;
            }
        }
        false
    }

    /// Get the struct definition with the given identifier.
    fn get_struct_definition_with_identifier(&self, id: &Identifier) -> Option<StructDefinition> {
        for m in self.modules.iter() {
            for s in m.borrow().structs.iter() {
                if &s.borrow().name == id {
                    return Some(s.borrow().clone());
                }
            }
        }
        None
    }

    /// Generate a function skeleton with name and signature.
    fn generate_function_skeleton(
        &self,
        u: &mut Unstructured,
        parent_scope: &Scope,
    ) -> Result<Function> {
        let (name, scope) = self.get_next_identifier(IdentifierType::Function, parent_scope);
        let signature = self.generate_function_signature(u, &scope, name)?;

        Ok(Function {
            signature,
            visibility: Visibility { public: true },
            body: None,
            return_stmt: None,
        })
    }

    /// Fill in the function body and return statement.
    fn fill_function(&self, u: &mut Unstructured, function: &RefCell<Function>) -> Result<()> {
        let scope = self
            .id_pool
            .borrow()
            .get_scope_for_children(&function.borrow().signature.name);
        let signature = function.borrow().signature.clone();
        let mut mut_func = function.borrow_mut();
        mut_func.body = Some(self.generate_function_body(u, &scope)?);
        mut_func.return_stmt = self.generate_return_stmt(u, &scope, &signature)?;
        Ok(())
    }

    /// Generate a function signature with random number of parameters and return type.
    fn generate_function_signature(
        &self,
        u: &mut Unstructured,
        parent_scope: &Scope,
        name: Identifier,
    ) -> Result<FunctionSignature> {
        let num_params = u.int_in_range(0..=self.config.max_num_params_in_func)?;
        let mut parameters = Vec::new();
        for _ in 0..num_params {
            let (name, _) = self.get_next_identifier(IdentifierType::Var, parent_scope);

            let typ = self.generate_basic_type(u)?;
            self.type_pool.borrow_mut().insert_mapping(&name, &typ);
            parameters.push((name, typ));
        }

        let return_type = match bool::arbitrary(u)? {
            true => Some(self.generate_basic_type(u)?),
            false => None,
        };

        Ok(FunctionSignature {
            name,
            parameters,
            return_type,
        })
    }

    /// Generate a return statement with a random expression.
    fn generate_return_stmt(
        &self,
        u: &mut Unstructured,
        parent_scope: &Scope,
        signature: &FunctionSignature,
    ) -> Result<Option<Expression>> {
        match signature.return_type {
            Some(ref typ) => {
                let ids = self.get_filtered_identifiers(
                    Some(typ),
                    Some(IdentifierType::Var),
                    Some(parent_scope),
                );
                match ids.is_empty() {
                    true => {
                        let expr =
                            self.generate_expression_of_type(u, parent_scope, typ, true, true)?;
                        Ok(Some(expr))
                    },
                    false => {
                        let ident = u.choose(&ids)?.clone();
                        Ok(Some(Expression::Variable(ident)))
                    },
                }
            },
            None => Ok(None),
        }
    }

    /// Generate a function body with random number of statements.
    fn generate_function_body(
        &self,
        u: &mut Unstructured,
        parent_scope: &Scope,
    ) -> Result<FunctionBody> {
        let len = u.int_in_range(0..=self.config.max_num_stmt_in_func)?;
        let mut stmts = Vec::new();

        for _ in 0..len {
            stmts.push(self.generate_statement(u, parent_scope)?);
        }

        Ok(FunctionBody { stmts })
    }

    /// Generate a random statement.
    fn generate_statement(&self, u: &mut Unstructured, parent_scope: &Scope) -> Result<Statement> {
        match u.int_in_range(0..=1)? {
            0 => Ok(Statement::Decl(self.generate_declaration(u, parent_scope)?)),
            1 => Ok(Statement::Expr(self.generate_expression(u, parent_scope)?)),
            _ => panic!("Invalid statement type"),
        }
    }

    /// Generate a random declaration.
    fn generate_declaration(
        &self,
        u: &mut Unstructured,
        parent_scope: &Scope,
    ) -> Result<Declaration> {
        let (name, _) = self.get_next_identifier(IdentifierType::Var, parent_scope);

        let typ = self.generate_basic_type(u)?;
        // let value = match bool::arbitrary(u)? {
        //     true => Some(self.generate_expression_of_type(u, parent_scope, &typ, true, true)?),
        //     false => None,
        // };
        // TODO: disabled declaration without value for now, need to keep track of initialization
        let value = Some(self.generate_expression_of_type(u, parent_scope, &typ, true, true)?);
        // Keeps track of the type of the newly created variable
        self.type_pool.borrow_mut().insert_mapping(&name, &typ);
        Ok(Declaration { typ, name, value })
    }

    /// Generate a random expression.
    fn generate_expression(
        &self,
        u: &mut Unstructured,
        parent_scope: &Scope,
    ) -> Result<Expression> {
        // If no function is callable, then skip generating function calls.
        let callable = self.get_callable_functions(parent_scope);
        let max = if callable.is_empty() { 1 } else { 2 };

        let expr = loop {
            match u.int_in_range(0..=max)? {
                // Generate a number literal
                0 => {
                    break Expression::NumberLiteral(self.generate_number_literal(
                        u,
                        parent_scope,
                        None,
                    )?)
                },
                // Generate a variable access
                1 => {
                    let idents = self.get_filtered_identifiers(
                        None,
                        Some(IdentifierType::Var),
                        Some(parent_scope),
                    );
                    if !idents.is_empty() {
                        let ident = u.choose(&idents)?.clone();
                        break Expression::Variable(ident);
                    }
                },
                // Generate a function call
                2 => {
                    let call = self.generate_function_call(u, parent_scope)?;
                    match call {
                        Some(c) => break Expression::FunctionCall(c),
                        None => panic!("No callable functions"),
                    }
                },
                _ => panic!("Invalid expression type"),
            }
        };
        Ok(expr)
    }

    /// Generate an expression of the given type.
    /// `allow_var`: allow using variable access, this is disabled for script
    /// `allow_call`: allow using function calls
    fn generate_expression_of_type(
        &self,
        u: &mut Unstructured,
        parent_scope: &Scope,
        typ: &Type,
        allow_var: bool,
        allow_call: bool,
    ) -> Result<Expression> {
        // Store candidate expressions for the given type
        let mut choices: Vec<Expression> = Vec::new();

        // Directly generate a value for basic types
        let candidate = match typ {
            Type::U8 | Type::U16 | Type::U32 | Type::U64 | Type::U128 | Type::U256 => {
                Expression::NumberLiteral(self.generate_number_literal(
                    u,
                    parent_scope,
                    Some(typ),
                )?)
            },
            Type::Bool => Expression::Boolean(bool::arbitrary(u)?),
            Type::Struct(id) => self.generate_struct_initialization(u, parent_scope, id)?,
            _ => unimplemented!(),
        };
        choices.push(candidate);

        // Access identifier with the given type
        if allow_var {
            let idents = self.get_filtered_identifiers(Some(typ), None, Some(parent_scope));

            // TODO: select from many?
            if !idents.is_empty() {
                let candidate = u.choose(&idents)?.clone();
                choices.push(Expression::Variable(candidate));
            }
        }

        // Call functions with the given return type
        if allow_call {
            let callables: Vec<FunctionSignature> = self
                .get_callable_functions(parent_scope)
                .into_iter()
                .filter(|f| f.return_type == Some(typ.clone()))
                .collect();
            // Currently, we generate calls to all candidate functions
            // This could consume a lot raw bytes and may interfere with mutation
            // TODO: consider just select a subset of functions to call
            if !callables.is_empty() {
                let func = u.choose(&callables)?;
                let call = self.generate_call_to_function(u, parent_scope, func, true)?;
                choices.push(Expression::FunctionCall(call));
            }
        }

        Ok(u.choose(&choices)?.clone())
    }

    /// Generate a struct initialization expression.
    /// This is `pack` in the parser AST.
    fn generate_struct_initialization(
        &self,
        u: &mut Unstructured,
        parent_scope: &Scope,
        struct_name: &Identifier,
    ) -> Result<Expression> {
        let struct_def = self
            .get_struct_definition_with_identifier(struct_name)
            .unwrap();

        let mut fields = Vec::new();
        for (name, typ) in struct_def.fields.iter() {
            let expr = self.generate_expression_of_type(u, parent_scope, typ, true, true)?;
            fields.push((name.clone(), expr));
        }
        Ok(Expression::StructInitialization(StructInitialization {
            name: struct_name.clone(),
            fields,
        }))
    }

    /// Generate a random function call.
    fn generate_function_call(
        &self,
        u: &mut Unstructured,
        parent_scope: &Scope,
    ) -> Result<Option<FunctionCall>> {
        let callables = self.get_callable_functions(parent_scope);
        if callables.is_empty() {
            return Ok(None);
        }

        let func = u.choose(&callables)?.clone();
        Ok(Some(self.generate_call_to_function(
            u,
            parent_scope,
            &func,
            true,
        )?))
    }

    /// Generate a call to the given function.
    fn generate_call_to_function(
        &self,
        u: &mut Unstructured,
        parent_scope: &Scope,
        func: &FunctionSignature,
        allow_var: bool,
    ) -> Result<FunctionCall> {
        let mut args = Vec::new();

        for (_, typ) in func.parameters.iter() {
            let expr = self.generate_expression_of_type(u, parent_scope, typ, allow_var, false)?;
            args.push(expr);
        }
        Ok(FunctionCall {
            name: func.name.clone(),
            args,
        })
    }

    /// Generate a random numerical literal.
    /// If the `typ` is `None`, a random type will be chosen.
    /// If the `typ` is `Some(Type::{U8, ..., U256})`, a literal of the given type will be used.
    fn generate_number_literal(
        &self,
        u: &mut Unstructured,
        _parent_scope: &Scope,
        typ: Option<&Type>,
    ) -> Result<NumberLiteral> {
        let idx = match typ {
            Some(t) => match t {
                Type::U8 => 0,
                Type::U16 => 1,
                Type::U32 => 2,
                Type::U64 => 3,
                Type::U128 => 4,
                Type::U256 => 5,
                _ => panic!("Invalid number literal type"),
            },
            None => u.int_in_range(0..=5)?,
        };

        Ok(match idx {
            0 => NumberLiteral {
                value: BigUint::from(u8::arbitrary(u)?),
                typ: Type::U8,
            },
            1 => NumberLiteral {
                value: BigUint::from(u16::arbitrary(u)?),
                typ: Type::U16,
            },
            2 => NumberLiteral {
                value: BigUint::from(u32::arbitrary(u)?),
                typ: Type::U32,
            },
            3 => NumberLiteral {
                value: BigUint::from(u64::arbitrary(u)?),
                typ: Type::U64,
            },
            4 => NumberLiteral {
                value: BigUint::from(u128::arbitrary(u)?),
                typ: Type::U128,
            },
            5 => NumberLiteral {
                value: BigUint::from_bytes_be(u.bytes(32)?),
                typ: Type::U256,
            },
            _ => panic!("Invalid number literal type"),
        })
    }

    /// Returns one of the basic types that does not require a type argument.
    pub fn generate_basic_type(&self, u: &mut Unstructured) -> Result<Type> {
        Ok(match u.int_in_range(0..=6)? {
            0 => Type::U8,
            1 => Type::U16,
            2 => Type::U32,
            3 => Type::U64,
            4 => Type::U128,
            5 => Type::U256,
            6 => Type::Bool,
            // x => Type::Address, // Leave these two until the end
            // x => Type::Signer,
            _ => panic!("Unsupported basic type"),
        })
    }

    /// Get all callable functions in the given scope.
    // TODO: Handle visibility check
    fn get_callable_functions(&self, scope: &Scope) -> Vec<FunctionSignature> {
        let mut callable = Vec::new();
        for f in self.function_signatures.iter() {
            let parent_scope = self.id_pool.borrow().get_parent_scope_of(&f.name).unwrap();
            if is_in_scope(scope, &parent_scope) {
                callable.push(f.clone());
            }
        }
        callable
    }

    /// Filter identifiers based on the given type, identifier type, and scope.
    fn get_filtered_identifiers(
        &self,
        typ: Option<&Type>,
        ident_type: Option<IdentifierType>,
        scope: Option<&Scope>,
    ) -> Vec<Identifier> {
        // Filter based on the IdentifierType
        let all_ident = match ident_type {
            Some(t) => self.id_pool.borrow().get_identifiers_of_ident_type(t),
            None => self.id_pool.borrow().get_all_identifiers(),
        };

        // Filter based on Scope
        let ident_in_scope = match scope {
            Some(s) => self
                .id_pool
                .borrow()
                .filter_identifier_in_scope(&all_ident, s),
            None => all_ident,
        };

        // Filter based on Type
        match typ {
            Some(t) => self
                .type_pool
                .borrow()
                .filter_identifier_with_type(t, ident_in_scope),
            None => ident_in_scope,
        }
    }

    /// Helper to get the next identifier.
    fn get_next_identifier(
        &self,
        ident_type: IdentifierType,
        parent_scope: &Scope,
    ) -> (Identifier, Scope) {
        self.id_pool
            .borrow_mut()
            .next_identifier(ident_type, parent_scope)
    }
}

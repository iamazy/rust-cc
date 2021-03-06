//!
//!
use crate::ast::{self, Ident};
use crate::types::{TypeRef,BaseType};

pub fn handle_function(program: &ast::Program, name: &str, ty: &crate::types::FunctionType, fcn_body: &mut ast::FunctionBody)
{
	debug!("handle_function({}: {:?})", name, ty);
	let mut ctx = Context {
		program: program,
		scopes: Default::default(),
		variables: Default::default(),
		ret_ty: ty.ret.clone(),
		};
	ctx.scopes.push(Default::default());
	for (ty, name) in &ty.args
	{
		ctx.define_variable(name.clone(), ty.clone());
	}

	ctx.visit_block(&mut fcn_body.code);

	// Save the variable list.
	for e in ctx.variables
	{
		fcn_body.var_table.push(ast::VarTableEnt {
			span: ast::Span,
			name: e.name,
			ty: e.ty,
			});
	}
}
pub fn handle_global(program: &ast::Program, name: &str, ty: &crate::types::TypeRef, val: &mut ast::Initialiser)
{
	debug!("handle_global({}: {:?}): val={:?}", name, ty, val);

	match val
	{
	ast::Initialiser::None => {},
	ast::Initialiser::Value(ref mut node) => {
		let mut ctx = Context {
			program: program,
			scopes: Default::default(),
			variables: Default::default(),
			ret_ty: ty.clone(),
			};
		ctx.visit_node(node, /*req_lvalue=*/false);
		},
	ast::Initialiser::ListLiteral(ref mut ents) => {
		for (i,e) in Iterator::enumerate(ents.iter_mut())
		{
			let mut ctx = Context {
				program: program,
				scopes: Default::default(),
				variables: Default::default(),
				ret_ty: match ty.basetype
					{
					BaseType::Array(ref inner, _) => inner.clone(),
					BaseType::Struct(ref s) =>
						match s.borrow().get_field_idx(i)
						{
						Some( (_, _, ty) ) => ty.clone(),
						None => panic!("Too many initialisers for struct"),
						},
					_ => todo!("List literal {:?}", ty),
					},
				};
			ctx.visit_node(e, /*req_lvalue=*/false);
		}
		},
	ast::Initialiser::ArrayLiteral(_) => todo!("handle_global({}: {:?}): val={:?}", name, ty, val),
	ast::Initialiser::StructLiteral(_) => todo!("handle_global({}: {:?}): val={:?}", name, ty, val),
	}
}

struct Context<'a>
{
	program: &'a ast::Program,
	ret_ty: TypeRef,
	scopes: Vec<VarScope>,
	variables: Vec<VarDef>,
}
#[derive(Default)]
struct VarScope
{
	variables: ::std::collections::HashMap<Ident, ValDef>,
}
enum ValDef
{
	Variable(usize),
}
struct VarDef
{
	name: Ident,
	ty: TypeRef,
}

impl<'a> Context<'a>
{
	fn define_variable(&mut self, name: Ident, ty: TypeRef) -> usize
	{
		let idx = self.variables.len();
		self.variables.push(VarDef {
			name: name.clone(),
			ty: ty,
			});
		let scope = self.scopes.last_mut().unwrap();
		if let Some(_v) = scope.variables.get(&name) {
			// TODO: This could be just a warning (C standard doesn't allow multiple definitions, but we could)
			panic!("Multiple defintions of `{}`", name);
		}
		scope.variables.insert(name, ValDef::Variable(idx));
		idx
	}

	fn visit_block(&mut self, blk: &mut ast::Block)
	{
		self.scopes.push(Default::default());
		for stmt in blk.iter_mut()
		{
			self.visit_stmt(stmt);
		}
		self.scopes.pop();
	}
	fn visit_stmt(&mut self, stmt: &mut ast::Statement)
	{
		use crate::ast::Statement;
		match *stmt
		{
		Statement::Empty => {},
		Statement::VarDef(ref mut list) => {
			for var_def in list
			{
				var_def.index = Some(self.define_variable(var_def.name.clone(), var_def.ty.clone()));
				self.visit_init(&var_def.ty, &mut var_def.value);
			}
			},
		Statement::Expr(ref mut e) => {
			trace!("{:?}", e);
			self.visit_node(e, false);
			},
		Statement::Block(ref mut stmts) => {
			self.visit_block(stmts);
			},

		// TODO: if and loops
		Statement::IfStatement { ref mut cond, ref mut true_arm, ref mut else_arm } => {
			trace!("if( {:?} )", cond);
			self.scopes.push(Default::default());
			self.visit_expr_def(cond);
			// TODO: Ensure that this is bool-like
			self.visit_block(true_arm);
			if let Some(else_arm) = else_arm {
				self.visit_block(else_arm);
			}
			self.scopes.pop();
			},
		Statement::WhileLoop { ref mut cond, ref mut body } => {
			trace!("while( {:?} )", cond);
			self.scopes.push(Default::default());
			self.visit_expr_def(cond);
			// TODO: Ensure that this is bool-like
			self.visit_block(body);
			self.scopes.pop();
			},
		Statement::DoWhileLoop { ref mut body, ref mut cond } => {
			trace!("do {{ ... }} while( {:?} )", cond);
			self.scopes.push(Default::default());
			self.visit_block(body);
			self.visit_node(cond, false);
			// TODO: Ensure that this is bool-like
			self.scopes.pop();
			},
		Statement::ForLoop { ref mut init, ref mut cond, ref mut inc, ref mut body } => {
			trace!("for( {:?}; {:?}; {:?} )", init, cond, inc);
			self.scopes.push(Default::default());
			if let Some(init) = init {
				self.visit_expr_def(init);
			}
			if let Some(cond) = cond {
				self.visit_node(cond, false);
				// TODO: Ensure that this is bool-like
			}
			if let Some(inc) = inc {
				self.visit_node(inc, false);
			}
			self.visit_block(body);
			self.scopes.pop();
			},

		Statement::Break => {
			// TODO: Check that there's a loop available
			},
		Statement::Continue => {
			// TODO: Check that there's a loop available
			},
		Statement::Return(ref mut opt_val) => {
			trace!("Return {:?}", opt_val);
			if let Some(val) = opt_val
			{
				// TODO: If type is void, use a different error
				self.visit_node(val, false);
				self.coerce_ty(&self.ret_ty, val);
			}
			else
			{
				// TODO: Check type is void
			}
			},

		Statement::Switch(ref mut val, ref mut body) => {
			trace!("switch {:?}", val);
			self.visit_node(val, false);
			// TODO: Check that the value is an int?
			// TODO: Push the value's type for match values?
			self.visit_block(body);
			},
		Statement::CaseDefault => {},
		Statement::CaseSingle(_v) => {},
		Statement::CaseRange(_v1, _v2) => {},

		Statement::Goto(ref _lbl) => {},
		Statement::Label(ref _lbl) => {},
		}
	}

	fn visit_expr_def(&mut self, ed: &mut ast::ExprOrDef)
	{
		match ed
		{
		ast::ExprOrDef::Expr(ref mut node) => self.visit_node(node, false),
		ast::ExprOrDef::Definition(ref mut defs) => {
			for var_def in defs
			{
				var_def.index = Some(self.define_variable(var_def.name.clone(), var_def.ty.clone()));
				self.visit_init(&var_def.ty, &mut var_def.value);
			}
			},
		}
	}

	fn visit_node(&mut self, node: &mut ast::Node, req_lvalue: bool)
	{
		let ty = self.visit_node_inner(&mut node.kind, req_lvalue);
		node.meta = Some(ast::NodeMeta {
			is_lvalue: req_lvalue,
			ty: ty,
			});
	}
	fn visit_node_inner(&mut self, node_kind: &mut ast::NodeKind, req_lvalue: bool) -> TypeRef
	{
		use crate::ast::NodeKind;
		match *node_kind
		{
		NodeKind::StmtList(ref mut nodes) => {
			let (last, nodes) = nodes.split_last_mut().unwrap();
			for n in nodes {
				self.visit_node(n, false);
			}
			self.visit_node(last, true);
			node_ty(last).clone()
			},
		NodeKind::Identifier(ref name, ref mut binding) => {
			for s in self.scopes.iter().rev()
			{
				if let Some(v) = s.variables.get(name)
				{
					let ty;
					*binding = Some(match *v
						{
						ValDef::Variable(idx) => {
							ty = self.variables[idx].ty.clone();
							crate::ast::IdentRef::Local(idx)
							},
						});
					return ty;
				}
			}
			// Search global scope (wait, shouldn't this have happened during parse?)
			if let Some(v) = self.program.get_symbol(name) {
				if let BaseType::Function(_) = v.symtype.basetype {
					// Special type for functions, as they have strange typecheck decay rules
					*binding = Some(ast::IdentRef::Function);
				}
				else {
					*binding = Some(ast::IdentRef::StaticItem);
				}
				return v.symtype.clone();
			}
			if let Some( (enm, idx) ) = self.program.find_enum_var(name) {
				*binding = Some(ast::IdentRef::Enum( enm, idx ));
				return crate::types::Type::new_ref_bare(BaseType::Integer(crate::types::IntClass::int()));
			}
			panic!("Unable to find '{}'", name);
			},
		NodeKind::String(_) => {
			if req_lvalue {
				self.err_no_lvalue();
			}
			crate::types::Type::new_ref_bare(BaseType::Pointer(
				crate::types::Type::new_ref(
					BaseType::Integer(crate::types::IntClass::Char(None)),
					{ let mut q = crate::types::Qualifiers::new(); q.set_const(); q }
					)
				))
			},
		NodeKind::Integer(_val, ty) => {
			if req_lvalue {
				self.err_no_lvalue();
			}
			crate::types::Type::new_ref_bare(BaseType::Integer(ty))
			},
		NodeKind::Float(_val, ty) => {
			if req_lvalue {
				self.err_no_lvalue();
			}
			crate::types::Type::new_ref_bare(BaseType::Float(ty))
			},

		NodeKind::FcnCall(ref mut fcn, ref mut args) => {
			if req_lvalue {
				self.err_no_lvalue();
			}
			self.visit_node(fcn, false);	// Does it need to be addressable?
			let fcn_ty = node_ty(&fcn);
			let fcn_ty = match fcn_ty.basetype
				{
				BaseType::Function(ref f) => f,
				_ => panic!("TODO: Error when calling a non-function. {:?}", fcn_ty),
				};
			for arg_val in args.iter_mut() {
				self.visit_node(arg_val, false);
			}
			for (arg_val, arg_ty_tup) in Iterator::zip( args.iter_mut(), fcn_ty.args.iter() )  {
				self.coerce_ty(&arg_ty_tup.0, arg_val);
			}
			// Variadic functions have `void` as the last arg
			if fcn_ty.is_variadic {
				for _ in args.iter_mut().skip(fcn_ty.args.len()) {
					// Any restriction on values?
					// - float must be double?
					// - integers must be `int` or larger
				}
			}

			fcn_ty.ret.clone()
			},

		NodeKind::Assign(ref mut slot, ref mut val) => {
			// NOTE: Allows lvalue
			self.visit_node(slot, true);
			self.visit_node(val, false);
			self.coerce_ty( node_ty(&slot), val );
			node_ty(&slot).clone()
			},
		NodeKind::AssignOp(ref _op, ref mut slot, ref mut val) => {
			// NOTE: Allows lvalue
			self.visit_node(slot, true);
			self.visit_node(val, false);
			let slot_ty = node_ty(&slot);
			if let BaseType::Pointer(..) = slot_ty.basetype
			{
				// TODO: Only add/sub allowed, and must be an integer (signed?)
			}
			else
			{
				self.coerce_ty(slot_ty, val);
			}
			// TODO: Ensure that the operation is valid for the type
			node_ty(&slot).clone()
			},
		NodeKind::Intrinsic(ref op, ref tys, ref mut vals) => match &op[..]
			{
			"va_start" => {
				if req_lvalue {
					self.err_no_lvalue();
				}
				for n in vals {
					self.visit_node(n, false);
				}
				//crate::types::c::void()
				crate::types::Type::new_ref_bare(BaseType::Void)
				},
			"va_end" => {
				if req_lvalue {
					self.err_no_lvalue();
				}
				for n in vals {
					self.visit_node(n, false);
				}
				//crate::types::c::void()
				crate::types::Type::new_ref_bare(BaseType::Void)
				},
			"va_arg" => {
				if req_lvalue {
					self.err_no_lvalue();
				}
				for n in vals {
					self.visit_node(n, false);
				}
				tys[0].clone()
				},
			_ => todo!("NodeKind::Intrinsic - {:?}", node_kind),
			}

		NodeKind::ImplicitCast(..) => panic!("Unexpected ImplicitCast in typecheck"),
		NodeKind::Cast(ref ty, ref mut val) => {
			if req_lvalue {
				self.err_no_lvalue();
			}
			self.visit_node(val, false);
			// TODO: Check cast validity
			ty.clone()
			},
		NodeKind::SizeofType(ref _ty) => {
			crate::types::Type::new_ref_bare(BaseType::Integer(
					crate::types::IntClass::Long(crate::types::Signedness::Unsigned)
					))
			},
		NodeKind::SizeofExpr(ref mut val) => {
			self.visit_node(val, false);
			crate::types::Type::new_ref_bare(BaseType::Integer(
					crate::types::IntClass::Long(crate::types::Signedness::Unsigned)
					))
			},

		// ...
		NodeKind::Ternary(ref mut cond, ref mut val_true, ref mut val_false) => {
			self.visit_node(cond, false);
			// TODO: Ensure that this is bool-like
			self.visit_node(val_true, req_lvalue);
			self.visit_node(val_false, req_lvalue);
			if node_ty(&val_true) != node_ty(&val_false) {
				todo!("Handle ternary type mismatch using promotion");
			}
			else {
				node_ty(&val_true).clone()
			}
			},
		NodeKind::UniOp(ref op, ref mut val) => {
			match op
			{
			// Pre/Post Inc/Dec require a lvalue
			ast::UniOp::PostDec
			| ast::UniOp::PreDec
			| ast::UniOp::PostInc
			| ast::UniOp::PreInc
				=> {
				// NOTE: Is LValue
				self.visit_node(val, true);	// needs a LValue
				node_ty(&val).clone()
				},
			ast::UniOp::Deref => {
				// NOTE: Is LValue
				self.visit_node(val, false);
				match node_ty(&val).deref()
				{
				Some(v) => v,
				None => panic!("Unable to deref {:?}", node_ty(&val)),
				}
				},
			ast::UniOp::Neg => {
				if req_lvalue {
					self.err_no_lvalue();
				}
				self.visit_node(val, false);
				// TODO: Check type (signed/float)
				node_ty(&val).clone()
				},
			ast::UniOp::BitNot => {
				if req_lvalue {
					self.err_no_lvalue();
				}
				self.visit_node(val, false);
				// TODO: Check type (unsigned only)
				node_ty(&val).clone()
				},
			ast::UniOp::LogicNot => {
				if req_lvalue {
					self.err_no_lvalue();
				}
				self.visit_node(val, false);
				// TODO: Check for bool-able
				crate::types::Type::new_ref_bare(BaseType::Bool)
				},
			ast::UniOp::Address => {
				if req_lvalue {
					self.err_no_lvalue();
				}
				self.visit_node(val, true);	// Needs a lvalue
				crate::types::Type::new_ref_bare( BaseType::Pointer(node_ty(&val).clone()) )
				},
			}
			},
		NodeKind::BinOp(ref op, ref mut val_l, ref mut val_r) => {
			if req_lvalue {
				self.err_no_lvalue();
			}
			self.visit_node(val_l, false);
			self.visit_node(val_r, false);

			if let BaseType::Array(ref inner, _) = node_ty(&val_l).basetype {
				let ptr_ty = crate::types::Type::new_ref(
					BaseType::Pointer(inner.clone()),
					node_ty(&val_l).qualifiers.clone(),
					);
				self.coerce_ty(&ptr_ty, val_l);
			}
			if let BaseType::Array(ref inner, _) = node_ty(&val_r).basetype {
				let ptr_ty = crate::types::Type::new_ref(
					BaseType::Pointer(inner.clone()),
					node_ty(&val_r).qualifiers.clone(),
					);
				self.coerce_ty(&ptr_ty, val_r);
			}

			use crate::ast::BinOp;
			match *op
			{
			BinOp::CmpEqu
			| BinOp::CmpNEqu
			| BinOp::CmpLt
			| BinOp::CmpLtE
			| BinOp::CmpGt
			| BinOp::CmpGtE
				=> {
					if node_ty(&val_l).basetype != node_ty(&val_r).basetype {
						let max = match self.max_ty( node_ty(&val_l), node_ty(&val_r))
							{
							Some(v) => v.clone(),
							None => todo!("Error with mismatched cmp types: {:?} {:?} and {:?}", op, val_l, val_r),
							};
						self.coerce_ty(&max, val_l);
						self.coerce_ty(&max, val_r);
					}
					crate::types::Type::new_ref_bare(BaseType::Bool)
				},
			BinOp::LogicAnd
			| BinOp::LogicOr
				=> {
					let ty_bool = crate::types::Type::new_ref_bare(BaseType::Bool);
					self.coerce_ty(&ty_bool, val_l);
					self.coerce_ty(&ty_bool, val_r);
					ty_bool
				},
			BinOp::ShiftLeft
			| BinOp::ShiftRight
				=> {
					if node_ty(&val_l).basetype != node_ty(&val_r).basetype {
						self.coerce_ty(node_ty(&val_l), val_r);
					}
					// TODO: Check that type is an integer
					node_ty(&val_l).clone()
				},
			BinOp::Mul
			| BinOp::Div
			| BinOp::Mod
			| BinOp::Add
			| BinOp::Sub
				=> {
					// If one side is a pointer, things get funny
					let ty_l = node_ty(&val_l);
					let ty_r = node_ty(&val_r);
					if let BaseType::Pointer(_) = ty_l.basetype {
						let ptrdiff_t = crate::types::Type::new_ref_bare(BaseType::Integer(
								crate::types::IntClass::Long(crate::types::Signedness::Signed)
								));
						if let BaseType::Pointer(_) = ty_r.basetype {
							ptrdiff_t
						}
						else {
							self.coerce_ty(&ptrdiff_t, val_r);
							ty_l.clone()
						}
					}
					else if let BaseType::Pointer(_) = ty_r.basetype {
						todo!("Handle RHS being a pointer but LHS not");
					}
					else if ty_l.basetype != ty_r.basetype {
						let max = match self.max_ty(ty_l, ty_r)
							{
							Some(v) => v.clone(),
							None => todo!("Error with mismatched binop types: {:?} {:?} and {:?}", op, val_l, val_r),
							};
						self.coerce_ty(&max, val_l);
						self.coerce_ty(&max, val_r);
						max
					}
					else {
						node_ty(&val_l).clone()
					}
				},
			BinOp::BitAnd
			| BinOp::BitOr
			| BinOp::BitXor
				=> {
					let ty_l = node_ty(&val_l);
					let ty_r = node_ty(&val_r);
					if ty_l.basetype != ty_r.basetype {
						let max = match self.max_ty(ty_l, ty_r)
							{
							Some(v) => v.clone(),
							None => todo!("Error with mismatched bitop types: {:?} {:?} and {:?}", op, val_l, val_r),
							};
						self.coerce_ty(&max, val_l);
						self.coerce_ty(&max, val_r);
						max
					}
					else {
						ty_l.clone()
					}
				},
			}
			},

		// TODO: This is actually `*(val + idx)` - should implement it as so
		NodeKind::Index(ref mut val, ref mut idx) => if false {
				// NOTE: Is always an LValue
				self.visit_node(val, false);	// Already a pointer, so will be LValue output
				self.visit_node(idx, false);
				node_ty(&val).deref().expect("Can't index")
			}
			else {
				// - Get the value and field name
				let (val, idx) = match *node_kind {
					NodeKind::Index(ref mut val, ref mut idx) => {
						(
							::std::mem::replace(val, Box::new( null_node() ) ),
							::std::mem::replace(idx, Box::new( null_node() ) ),
							)
						},
					_ => unreachable!(),
					};
				// - Update the node to be `(*val).NAME`
				*node_kind = NodeKind::UniOp(ast::UniOp::Deref,
					Box::new(ast::Node::new(NodeKind::BinOp(ast::BinOp::Add, val, idx)))
					);
				// - Recurse
				self.visit_node_inner(node_kind, req_lvalue)
			},
		// Implemented as `(*val).NAME` (using replacement)
		NodeKind::DerefMember(..) => {
			// - Get the value and field name
			let (val, name) = match *node_kind {
				NodeKind::DerefMember(ref mut val, ref mut name) => {
					(
						::std::mem::replace(val, Box::new( null_node() ) ),
						::std::mem::replace(name, Ident::new()),
						)
					},
				_ => unreachable!(),
				};
			// - Update the node to be `(*val).NAME`
			*node_kind = NodeKind::Member(
				Box::new( ast::Node::new(NodeKind::UniOp(ast::UniOp::Deref, val)) ),
				name
				);
			// - Recurse
			self.visit_node_inner(node_kind, req_lvalue)
			},
		NodeKind::Member(ref mut val, ref name) => {
			self.visit_node(val, req_lvalue);
			match node_ty(&val).get_field(name)
			{
			None => panic!("Unable to find field"),
			Some( (_ofs, ty) ) => ty,
			}
			},
		}
	}

	fn visit_init(&mut self, exp_ty: &TypeRef, init: &mut ast::Initialiser)
	{
		use crate::ast::Initialiser;
		match *init
		{
		Initialiser::None => {},
		Initialiser::Value(ref mut node) => {
			self.visit_node(node, false);
			// TODO: Check expected type
			},
		_ => todo!("visit_init({:?}):, {:?}", exp_ty, init),
		}
	}

	fn max_ty(&self, ty1: &TypeRef, ty2: &TypeRef) -> Option<TypeRef> {
		use crate::types::{Signedness,IntClass};
		fn sgn(s1: &Signedness, s2: &Signedness) -> Signedness {
			match (s1,s2)
			{
			(Signedness::Unsigned, Signedness::Unsigned) => Signedness::Unsigned,
			_ => Signedness::Signed,
			}
		}
		Some(crate::types::Type::new_ref_bare(match (&ty1.basetype, &ty2.basetype)
		{
		(BaseType::Integer(i1), BaseType::Integer(i2)) => BaseType::Integer(match i1
			{
			//IntClass::Bits(_s1, _n) => todo!("max_ty Integers {:?} {:?}", i1, i2),
			IntClass::Char(_s1) => match i2
				{
				//IntClass::Bits(_s2, _n) => todo!("max_ty Integers {:?} {:?}", i1, i2),
				_ => i2.clone_with_sgn( sgn(&i1.signedness(), &i2.signedness()) ),
				},
			IntClass::Short(s1) => match i2
				{
				//IntClass::Bits(_s2, _n) => todo!("max_ty Integers {:?} {:?}", i1, i2),
				IntClass::LongLong(s2) => i2.clone_with_sgn( sgn(s1, s2) ),
				IntClass::Long(s2) => i2.clone_with_sgn( sgn(s1, s2) ),
				IntClass::Int(s2) => i2.clone_with_sgn( sgn(s1, s2) ),
				_ => i1.clone_with_sgn( sgn(s1, &i2.signedness()) ),
				},
			IntClass::Int(s1) => match i2
				{
				//IntClass::Bits(_s2, _n) => todo!("max_ty Integers {:?} {:?}", i1, i2),
				IntClass::LongLong(s2) => i2.clone_with_sgn( sgn(s1, s2) ),
				IntClass::Long(s2) => i2.clone_with_sgn( sgn(s1, s2) ),
				_ => i1.clone_with_sgn( sgn(s1, &i2.signedness()) ),
				},
			IntClass::Long(s1) => match i2
				{
				//IntClass::Bits(_s2, _n) => todo!("max_ty Integers {:?} {:?}", i1, i2),
				IntClass::LongLong(s2) => i2.clone_with_sgn( sgn(s1, s2) ),
				_ => i1.clone_with_sgn( sgn(s1, &i2.signedness()) ),
				},
			IntClass::LongLong(s1) => match i2
				{
				//IntClass::Bits(_s2, _n) => todo!("max_ty Integers {:?} {:?}", i1, i2),
				_ => i1.clone_with_sgn( sgn(s1, &i2.signedness()) ),
				},
			}),
		(BaseType::Pointer(i1), BaseType::Pointer(i2)) => BaseType::Pointer({
			if i1.basetype != i2.basetype {
				todo!("Pick 'max' of {:?} and {:?} - Mismatched pointer inner", ty1, ty2);
			}
			let mut q = i1.qualifiers.clone();
			q.merge_from(&i2.qualifiers);
			crate::types::Type::new_ref(i1.basetype.clone(), q)
			}),
		_ => todo!("Pick 'max' of {:?} and {:?}", ty1, ty2),
		}))
	}
	fn coerce_ty(&self, req_ty: &TypeRef, node: &mut ast::Node)
	{
		if req_ty.basetype != node_ty(&node).basetype {
			let inner_node = ::std::mem::replace(node, null_node());
			*node = ast::Node::new(ast::NodeKind::ImplicitCast(req_ty.clone(), Box::new(inner_node)));
			node.meta = Some(ast::NodeMeta {
				is_lvalue: false,
				ty: req_ty.clone(),
				});
			let inner_ty = match node.kind
				{
				ast::NodeKind::ImplicitCast(_, ref node) => node_ty(&node),
				_ => unreachable!(),
				};
			match req_ty.basetype
			{
			BaseType::Bool => match inner_ty.basetype
				{
				BaseType::Bool => {},
				BaseType::Integer(_) => {},
				BaseType::Float(_) => {},
				BaseType::Pointer(..) => {},
				_ => todo!("Handle type mismatch using promotion/demotion of value: {:?} from {:?}", req_ty, inner_ty),
				},
			BaseType::Integer(_ic) => match inner_ty.basetype
				{
				BaseType::Bool => {},
				BaseType::Integer(_ici) => {},	// TODO: Warn on signed-ness?
				_ => todo!("Handle type mismatch using promotion/demotion of value: {:?} from {:?}", req_ty, inner_ty),
				},
			BaseType::Pointer(ref _i1) => match inner_ty.basetype
				{
				BaseType::Pointer(ref _i2) => {},	// TODO: Const/restrict/etc warnings
				BaseType::Array(_, _) => {},	// TODO: Const/restrict/etc warnings
				_ => todo!("Handle type mismatch using promotion/demotion of value: {:?} from {:?}", req_ty, inner_ty),
				},
			_ => todo!("Handle type mismatch using promotion/demotion of value: {:?} from {:?}", req_ty, inner_ty),
			}
		}
	}

	fn err_no_lvalue(&self)
	{
		panic!("Unexpected node in lvalue");
	}
}

fn node_ty(n: &ast::Node) -> &TypeRef {
	&n.meta.as_ref().unwrap().ty
}

fn null_node() -> ast::Node {
	ast::Node::new(ast::NodeKind::StmtList(vec![]))
}

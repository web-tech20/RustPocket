use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::time::Instant;

use proc_macro2::TokenStream;
use syn::spanned::Spanned;
use syn::{
    BinOp, Expr, ExprLit, Item, Lit, Pat, Stmt, UnOp,
};

use crate::checker::{check_rust, render_error};
use crate::models::{Diagnostic, RunResult};

const MAX_STEPS: usize = 500_000;
const MAX_DEPTH: usize = 200;
const MAX_OUTPUT_BYTES: usize = 64 * 1024;

#[derive(Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Char(char),
    Str(String),
    Unit,
    Tuple(Vec<Value>),
    Array(Vec<Value>),
    Range { start: i64, end: i64, inclusive: bool },
    Struct { name: String, fields: HashMap<String, Value> },
    Enum { name: String, variant: String, payload: Vec<Value> },
    Option(Option<Box<Value>>),
    Result(std::result::Result<Box<Value>, Box<Value>>),
    Map(HashMap<String, Value>),
    Stdin,
    Stdout,
    Command {
        cmd: String,
        args_list: Vec<String>,
        envs: Vec<(String, String)>,
    },
    Closure {
        params: Vec<String>,
        body: Box<Expr>,
        captured: HashMap<String, Value>,
    },
}

impl Value {
    fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "i64",
            Value::Float(_) => "f64",
            Value::Bool(_) => "bool",
            Value::Char(_) => "char",
            Value::Str(_) => "String",
            Value::Unit => "()",
            Value::Tuple(_) => "tuple",
            Value::Array(_) => "Vec",
            Value::Range { .. } => "Range",
            Value::Struct { .. } => "struct",
            Value::Enum { .. } => "enum",
            Value::Option(_) => "Option",
            Value::Result(_) => "Result",
            Value::Map(_) => "HashMap",
            Value::Stdin => "Stdin",
            Value::Stdout => "Stdout",
            Value::Command { .. } => "Command",
            Value::Closure { .. } => "closure",
        }
    }

    fn display_str(&self) -> String {
        match self {
            Value::Int(n) => n.to_string(),
            Value::Float(f) => {
                if f.fract() == 0.0 && f.is_finite() {
                    format!("{f:.1}")
                } else {
                    f.to_string()
                }
            }
            Value::Bool(b) => b.to_string(),
            Value::Char(c) => c.to_string(),
            Value::Str(s) => s.clone(),
            Value::Unit => "()".to_string(),
            _ => self.debug_str(),
        }
    }

    fn debug_str(&self) -> String {
        match self {
            Value::Int(n) => n.to_string(),
            Value::Float(f) => format!("{f:?}"),
            Value::Bool(b) => b.to_string(),
            Value::Char(c) => format!("{c:?}"),
            Value::Str(s) => format!("{s:?}"),
            Value::Unit => "()".to_string(),
            Value::Tuple(items) => {
                let inner = items.iter().map(Value::debug_str).collect::<Vec<_>>().join(", ");
                if items.len() == 1 {
                    format!("({inner},)")
                } else {
                    format!("({inner})")
                }
            }
            Value::Array(items) => {
                let inner = items.iter().map(Value::debug_str).collect::<Vec<_>>().join(", ");
                format!("[{inner}]")
            }
            Value::Range { start, end, inclusive } => {
                if *inclusive {
                    format!("{start}..={end}")
                } else {
                    format!("{start}..{end}")
                }
            }
            Value::Struct { name, fields } => {
                let mut pairs: Vec<_> = fields.iter().collect();
                pairs.sort_by_key(|(k, _)| *k);
                let inner = pairs
                    .into_iter()
                    .map(|(k, v)| format!("{k}: {}", v.debug_str()))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{name} {{ {inner} }}")
            }
            Value::Enum { variant, payload, .. } => {
                if payload.is_empty() {
                    variant.clone()
                } else {
                    let inner = payload.iter().map(Value::debug_str).collect::<Vec<_>>().join(", ");
                    format!("{variant}({inner})")
                }
            }
            Value::Option(Some(v)) => format!("Some({})", v.debug_str()),
            Value::Option(None) => "None".to_string(),
            Value::Result(Ok(v)) => format!("Ok({})", v.debug_str()),
            Value::Result(Err(e)) => format!("Err({})", e.debug_str()),
            Value::Map(m) => {
                let mut pairs: Vec<_> = m.iter().collect();
                pairs.sort_by_key(|(k, _)| *k);
                let inner = pairs
                    .into_iter()
                    .map(|(k, v)| format!("{k:?}: {}", v.debug_str()))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{{inner}}}")
            }
            Value::Stdin => "Stdin { .. }".to_string(),
            Value::Stdout => "Stdout { .. }".to_string(),
            Value::Command { cmd, .. } => format!("Command({cmd:?})"),
            Value::Closure { .. } => "<closure>".to_string(),
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.debug_str())
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display_str())
    }
}

#[derive(Debug)]
enum ControlFlow {
    Return(Value),
    Break(Option<Value>),
    Continue,
    NeedsInput,
    Error(RuntimeError),
}

#[derive(Debug)]
struct RuntimeError {
    message: String,
    line: Option<usize>,
    column: Option<usize>,
    timed_out: bool,
}

impl RuntimeError {
    fn new(message: impl Into<String>, span: proc_macro2::Span) -> Self {
        let start = span.start();
        Self {
            message: message.into(),
            line: if start.line > 0 { Some(start.line) } else { None },
            column: if start.line > 0 { Some(start.column + 1) } else { None },
            timed_out: false,
        }
    }

    fn timeout() -> Self {
        Self {
            message: "Temps d'exécution dépassé (limite d'itérations atteinte).".to_string(),
            line: None,
            column: None,
            timed_out: true,
        }
    }
}

impl From<RuntimeError> for ControlFlow {
    fn from(err: RuntimeError) -> Self {
        ControlFlow::Error(err)
    }
}

#[derive(Clone)]
struct FnDef {
    params: Vec<String>,
    has_self: bool,
    body: syn::Block,
}

struct Env {
    scopes: Vec<HashMap<String, Value>>,
}

impl Env {
    fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    fn push(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    fn define(&mut self, name: String, value: Value) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, value);
        }
    }

    fn get(&self, name: &str) -> Option<&Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v);
            }
        }
        None
    }

    fn set(&mut self, name: &str, value: Value) -> bool {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(slot) = scope.get_mut(name) {
                *slot = value;
                return true;
            }
        }
        false
    }

    fn snapshot(&self) -> HashMap<String, Value> {
        let mut map = HashMap::new();
        for scope in &self.scopes {
            for (k, v) in scope {
                map.insert(k.clone(), v.clone());
            }
        }
        map
    }
}

struct Interpreter {
    functions: HashMap<String, FnDef>,
    methods: HashMap<String, HashMap<String, FnDef>>,
    consts: HashMap<String, Value>,
    enum_variants: HashMap<String, String>,
    stdin_lines: VecDeque<String>,
    needs_input: bool,
    stdout: String,
    stderr: String,
    truncated: bool,
    steps: usize,
    depth: usize,
}

impl Interpreter {
    fn new(stdin_text: &str) -> Self {
        let mut stdin_lines = VecDeque::new();
        if !stdin_text.is_empty() {
            for line in stdin_text.split_inclusive('\n') {
                stdin_lines.push_back(line.to_string());
            }
            if !stdin_text.ends_with('\n') {
                if let Some(last) = stdin_lines.back_mut() {
                    last.push('\n');
                }
            }
        }
        Self {
            functions: HashMap::new(),
            methods: HashMap::new(),
            consts: HashMap::new(),
            enum_variants: HashMap::new(),
            stdin_lines,
            needs_input: false,
            stdout: String::new(),
            stderr: String::new(),
            truncated: false,
            steps: 0,
            depth: 0,
        }
    }

    fn tick(&mut self) -> Result<(), ControlFlow> {
        self.steps += 1;
        if self.steps > MAX_STEPS {
            return Err(ControlFlow::Error(RuntimeError::timeout()));
        }
        Ok(())
    }

    fn write_stdout(&mut self, text: &str) {
        if self.stdout.len() + text.len() > MAX_OUTPUT_BYTES {
            let remaining = MAX_OUTPUT_BYTES.saturating_sub(self.stdout.len());
            if remaining > 0 {
                let mut end = remaining;
                while end > 0 && !text.is_char_boundary(end) {
                    end -= 1;
                }
                self.stdout.push_str(&text[..end]);
            }
            self.truncated = true;
        } else {
            self.stdout.push_str(text);
        }
    }

    fn write_stderr(&mut self, text: &str) {
        if self.stderr.len() + text.len() > MAX_OUTPUT_BYTES {
            let remaining = MAX_OUTPUT_BYTES.saturating_sub(self.stderr.len());
            if remaining > 0 {
                let mut end = remaining;
                while end > 0 && !text.is_char_boundary(end) {
                    end -= 1;
                }
                self.stderr.push_str(&text[..end]);
            }
            self.truncated = true;
        } else {
            self.stderr.push_str(text);
        }
    }

    fn load_file(&mut self, file: &syn::File) -> Result<(), ControlFlow> {
        let mut const_items = Vec::new();
        for item in &file.items {
            match item {
                Item::Fn(f) => {
                    let def = parse_fn_def(f);
                    self.functions.insert(f.sig.ident.to_string(), def);
                }
                Item::Impl(imp) => {
                    if let syn::Type::Path(tp) = imp.self_ty.as_ref() {
                        if let Some(seg) = tp.path.segments.last() {
                            let type_name = seg.ident.to_string();
                            let entry = self.methods.entry(type_name).or_default();
                            for impl_item in &imp.items {
                                if let syn::ImplItem::Fn(m) = impl_item {
                                    let (params, has_self) = parse_sig_params(&m.sig);
                                    entry.insert(
                                        m.sig.ident.to_string(),
                                        FnDef {
                                            params,
                                            has_self,
                                            body: m.block.clone(),
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
                Item::Enum(en) => {
                    let enum_name = en.ident.to_string();
                    for v in &en.variants {
                        self.enum_variants
                            .insert(v.ident.to_string(), enum_name.clone());
                    }
                }
                Item::Const(c) => {
                    const_items.push(c);
                }
                _ => {}
            }
        }
        let mut env = Env::new();
        for c in const_items {
            let val = self.eval_expr(&c.expr, &mut env)?;
            self.consts.insert(c.ident.to_string(), val);
        }
        Ok(())
    }

    fn call_fn(&mut self, name: &str, args: Vec<Value>, span: proc_macro2::Span) -> Result<Value, ControlFlow> {
        let def = self
            .functions
            .get(name)
            .cloned()
            .ok_or_else(|| RuntimeError::new(format!("fonction `{name}` introuvable"), span))?;
        self.invoke_fn_def(&def, None, args, span)
    }

    fn invoke_fn_def(
        &mut self,
        def: &FnDef,
        self_val: Option<Value>,
        args: Vec<Value>,
        span: proc_macro2::Span,
    ) -> Result<Value, ControlFlow> {
        if self.depth >= MAX_DEPTH {
            return Err(RuntimeError::new(
                "Profondeur de récursion maximale atteinte (stack overflow).",
                span,
            )
            .into());
        }
        if args.len() != def.params.len() {
            return Err(RuntimeError::new(
                format!(
                    "nombre d'arguments incorrect : attendu {}, reçu {}",
                    def.params.len(),
                    args.len()
                ),
                span,
            )
            .into());
        }
        self.depth += 1;
        let mut env = Env::new();
        for (k, v) in &self.consts {
            env.define(k.clone(), v.clone());
        }
        if let Some(sv) = self_val {
            env.define("self".to_string(), sv);
        }
        for (param, val) in def.params.iter().zip(args) {
            env.define(param.clone(), val);
        }
        let res = self.eval_block(&def.body, &mut env);
        self.depth -= 1;
        match res {
            Ok(val) => Ok(val),
            Err(ControlFlow::Return(val)) => Ok(val),
            Err(other) => Err(other),
        }
    }

    fn eval_block(&mut self, block: &syn::Block, env: &mut Env) -> Result<Value, ControlFlow> {
        env.push();
        let mut last = Value::Unit;
        for stmt in &block.stmts {
            self.tick()?;
            match stmt {
                Stmt::Local(local) => {
                    let val = if let Some(init) = &local.init {
                        self.eval_expr(&init.expr, env)?
                    } else {
                        Value::Unit
                    };
                    bind_pattern(&local.pat, val, env)?;
                }
                Stmt::Item(Item::Fn(f)) => {
                    self.functions.insert(f.sig.ident.to_string(), parse_fn_def(f));
                }
                Stmt::Item(Item::Const(c)) => {
                    let val = self.eval_expr(&c.expr, env)?;
                    env.define(c.ident.to_string(), val);
                }
                Stmt::Item(_) => {}
                Stmt::Expr(expr, semi) => {
                    let val = match self.eval_expr(expr, env) {
                        Ok(v) => v,
                        Err(e) => {
                            env.pop();
                            return Err(e);
                        }
                    };
                    last = if semi.is_some() { Value::Unit } else { val };
                }
                Stmt::Macro(m) => {
                    let name = m.mac.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
                    if let Err(e) = self.eval_macro(&name, &m.mac.tokens, m.span(), env) {
                        env.pop();
                        return Err(e);
                    }
                    last = Value::Unit;
                }
            }
        }
        env.pop();
        Ok(last)
    }

    fn eval_expr(&mut self, expr: &Expr, env: &mut Env) -> Result<Value, ControlFlow> {
        self.tick()?;
        match expr {
            Expr::Lit(ExprLit { lit, .. }) => eval_lit(lit, expr.span()),
            Expr::Paren(p) => self.eval_expr(&p.expr, env),
            Expr::Group(g) => self.eval_expr(&g.expr, env),
            Expr::Block(b) => self.eval_block(&b.block, env),
            Expr::Path(p) => {
                let segs: Vec<String> = p.path.segments.iter().map(|s| s.ident.to_string()).collect();
                if segs.len() == 1 {
                    let name = &segs[0];
                    if name == "None" {
                        return Ok(Value::Option(None));
                    }
                    if let Some(v) = env.get(name) {
                        return Ok(v.clone());
                    }
                    if let Some(v) = self.consts.get(name) {
                        return Ok(v.clone());
                    }
                    if let Some(enum_name) = self.enum_variants.get(name) {
                        return Ok(Value::Enum {
                            name: enum_name.clone(),
                            variant: name.clone(),
                            payload: vec![],
                        });
                    }
                    Err(RuntimeError::new(format!("variable `{name}` non définie"), expr.span()).into())
                } else if segs.len() == 2 {
                    let (ty, variant) = (&segs[0], &segs[1]);
                    if ty == "Option" && variant == "None" {
                        return Ok(Value::Option(None));
                    }
                    Ok(Value::Enum {
                        name: ty.clone(),
                        variant: variant.clone(),
                        payload: vec![],
                    })
                } else {
                    Err(RuntimeError::new("chemin non supporté", expr.span()).into())
                }
            }
            Expr::Unary(u) => self.eval_unary(u, env),
            Expr::Binary(b) => self.eval_binary(b, env),
            Expr::Assign(a) => self.eval_assign(&a.left, &a.right, None, env),
            Expr::Cast(c) => {
                let val = self.eval_expr(&c.expr, env)?;
                eval_cast(val, &c.ty, expr.span())
            }
            Expr::Tuple(t) => {
                let mut items = Vec::with_capacity(t.elems.len());
                for elem in &t.elems {
                    items.push(self.eval_expr(elem, env)?);
                }
                Ok(Value::Tuple(items))
            }
            Expr::Array(a) => {
                let mut items = Vec::with_capacity(a.elems.len());
                for elem in &a.elems {
                    items.push(self.eval_expr(elem, env)?);
                }
                Ok(Value::Array(items))
            }
            Expr::Repeat(r) => {
                let val = self.eval_expr(&r.expr, env)?;
                let len = match self.eval_expr(&r.len, env)? {
                    Value::Int(n) if n >= 0 => n as usize,
                    _ => return Err(RuntimeError::new("taille de tableau invalide", r.len.span()).into()),
                };
                Ok(Value::Array(vec![val; len.min(100_000)]))
            }
            Expr::Range(r) => {
                let start = match &r.start {
                    Some(s) => match self.eval_expr(s, env)? {
                        Value::Int(n) => n,
                        _ => return Err(RuntimeError::new("borne de plage entière attendue", s.span()).into()),
                    },
                    None => 0,
                };
                let end = match &r.end {
                    Some(e) => match self.eval_expr(e, env)? {
                        Value::Int(n) => n,
                        _ => return Err(RuntimeError::new("borne de plage entière attendue", e.span()).into()),
                    },
                    None => 0,
                };
                Ok(Value::Range {
                    start,
                    end,
                    inclusive: matches!(r.limits, syn::RangeLimits::Closed(_)),
                })
            }
            Expr::Index(idx) => {
                let target = self.eval_expr(&idx.expr, env)?;
                let index_val = self.eval_expr(&idx.index, env)?;
                match (target, index_val) {
                    (Value::Array(items), Value::Int(i)) => {
                        if i < 0 || (i as usize) >= items.len() {
                            Err(RuntimeError::new(
                                format!("index {i} hors limites (longueur {})", items.len()),
                                expr.span(),
                            )
                            .into())
                        } else {
                            Ok(items[i as usize].clone())
                        }
                    }
                    (Value::Array(items), Value::Range { start, end, inclusive }) => {
                        let s = start.max(0) as usize;
                        let e = if inclusive { (end + 1).max(0) as usize } else { end.max(0) as usize };
                        if s <= e && e <= items.len() {
                            Ok(Value::Array(items[s..e].to_vec()))
                        } else {
                            Err(RuntimeError::new("tranche (slice) hors limites", expr.span()).into())
                        }
                    }
                    _ => Err(RuntimeError::new("indexation invalide", expr.span()).into()),
                }
            }
            Expr::Field(f) => {
                let base = self.eval_expr(&f.base, env)?;
                match (&base, &f.member) {
                    (Value::Struct { fields, name }, syn::Member::Named(ident)) => {
                        let key = ident.to_string();
                        fields.get(&key).cloned().ok_or_else(|| {
                            RuntimeError::new(format!("champ `{key}` introuvable sur `{name}`"), f.span()).into()
                        })
                    }
                    (Value::Tuple(items), syn::Member::Unnamed(idx)) => {
                        items.get(idx.index as usize).cloned().ok_or_else(|| {
                            RuntimeError::new("index de tuple hors limites", f.span()).into()
                        })
                    }
                    _ => Err(RuntimeError::new("accès au champ impossible", f.span()).into()),
                }
            }
            Expr::Struct(s) => {
                let name = s.path.segments.last().map(|seg| seg.ident.to_string()).unwrap_or_default();
                let mut fields = if let Some(rest) = &s.rest {
                    match self.eval_expr(rest, env)? {
                        Value::Struct { fields, .. } => fields,
                        _ => HashMap::new(),
                    }
                } else {
                    HashMap::new()
                };
                for fv in &s.fields {
                    let key = match &fv.member {
                        syn::Member::Named(id) => id.to_string(),
                        syn::Member::Unnamed(idx) => idx.index.to_string(),
                    };
                    let val = self.eval_expr(&fv.expr, env)?;
                    fields.insert(key, val);
                }
                Ok(Value::Struct { name, fields })
            }
            Expr::If(if_expr) => self.eval_if(if_expr, env),
            Expr::While(w) => self.eval_while(w, env),
            Expr::Loop(l) => self.eval_loop(l, env),
            Expr::ForLoop(fl) => self.eval_for(fl, env),
            Expr::Match(m) => self.eval_match(m, env),
            Expr::Break(b) => {
                let val = match &b.expr {
                    Some(e) => Some(self.eval_expr(e, env)?),
                    None => None,
                };
                Err(ControlFlow::Break(val))
            }
            Expr::Continue(_) => Err(ControlFlow::Continue),
            Expr::Return(r) => {
                let val = match &r.expr {
                    Some(e) => self.eval_expr(e, env)?,
                    None => Value::Unit,
                };
                Err(ControlFlow::Return(val))
            }
            Expr::Call(c) => self.eval_call(c, env),
            Expr::MethodCall(mc) => self.eval_method_call(mc, env),
            Expr::Macro(m) => {
                let name = m.mac.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
                self.eval_macro(&name, &m.mac.tokens, m.span(), env)
            }
            Expr::Closure(cl) => {
                let mut params = Vec::with_capacity(cl.inputs.len());
                for pat in &cl.inputs {
                    params.push(pat_ident_name(pat).unwrap_or_else(|| "_".to_string()));
                }
                Ok(Value::Closure {
                    params,
                    body: cl.body.clone(),
                    captured: env.snapshot(),
                })
            }
            Expr::Reference(r) => self.eval_expr(&r.expr, env),
            Expr::Await(a) => self.eval_expr(&a.base, env),
            Expr::Async(a) => self.eval_block(&a.block, env),
            Expr::Try(t) => match self.eval_expr(&t.expr, env)? {
                Value::Option(Some(v)) => Ok(*v),
                Value::Option(None) => Err(ControlFlow::Return(Value::Option(None))),
                Value::Result(Ok(v)) => Ok(*v),
                Value::Result(Err(e)) => Err(ControlFlow::Return(Value::Result(Err(e)))),
                other => Err(RuntimeError::new(
                    format!("l'opérateur `?` nécessite Option ou Result, trouvé {}", other.type_name()),
                    t.span(),
                )
                .into()),
            },
            Expr::Let(l) => {
                let val = self.eval_expr(&l.expr, env)?;
                if match_pattern(&l.pat, &val, env) {
                    Ok(Value::Bool(true))
                } else {
                    Ok(Value::Bool(false))
                }
            }
            _ => Err(RuntimeError::new("expression Rust non supportée par l'interpréteur embarqué", expr.span()).into()),
        }
    }

    fn eval_unary(&mut self, u: &syn::ExprUnary, env: &mut Env) -> Result<Value, ControlFlow> {
        let val = self.eval_expr(&u.expr, env)?;
        match u.op {
            UnOp::Neg(_) => match val {
                Value::Int(n) => Ok(Value::Int(n.wrapping_neg())),
                Value::Float(f) => Ok(Value::Float(-f)),
                _ => Err(RuntimeError::new("négation unaire invalide", u.span()).into()),
            },
            UnOp::Not(_) => match val {
                Value::Bool(b) => Ok(Value::Bool(!b)),
                Value::Int(n) => Ok(Value::Int(!n)),
                _ => Err(RuntimeError::new("opérateur `!` invalide", u.span()).into()),
            },
            UnOp::Deref(_) => Ok(val),
            _ => Err(RuntimeError::new("opérateur unaire non supporté", u.span()).into()),
        }
    }

    fn eval_binary(&mut self, b: &syn::ExprBinary, env: &mut Env) -> Result<Value, ControlFlow> {
        // Compound assignments (+=, -=, *=, /=, %=)
        match b.op {
            BinOp::AddAssign(_) => return self.eval_assign(&b.left, &b.right, Some(CompoundOp::Add), env),
            BinOp::SubAssign(_) => return self.eval_assign(&b.left, &b.right, Some(CompoundOp::Sub), env),
            BinOp::MulAssign(_) => return self.eval_assign(&b.left, &b.right, Some(CompoundOp::Mul), env),
            BinOp::DivAssign(_) => return self.eval_assign(&b.left, &b.right, Some(CompoundOp::Div), env),
            BinOp::RemAssign(_) => return self.eval_assign(&b.left, &b.right, Some(CompoundOp::Rem), env),
            BinOp::And(_) => {
                let lhs = self.eval_expr(&b.left, env)?;
                return match lhs {
                    Value::Bool(false) => Ok(Value::Bool(false)),
                    Value::Bool(true) => match self.eval_expr(&b.right, env)? {
                        Value::Bool(r) => Ok(Value::Bool(r)),
                        _ => Err(RuntimeError::new("booléen attendu après `&&`", b.right.span()).into()),
                    },
                    _ => Err(RuntimeError::new("booléen attendu avant `&&`", b.left.span()).into()),
                };
            }
            BinOp::Or(_) => {
                let lhs = self.eval_expr(&b.left, env)?;
                return match lhs {
                    Value::Bool(true) => Ok(Value::Bool(true)),
                    Value::Bool(false) => match self.eval_expr(&b.right, env)? {
                        Value::Bool(r) => Ok(Value::Bool(r)),
                        _ => Err(RuntimeError::new("booléen attendu après `||`", b.right.span()).into()),
                    },
                    _ => Err(RuntimeError::new("booléen attendu avant `||`", b.left.span()).into()),
                };
            }
            _ => {}
        }

        let lhs = self.eval_expr(&b.left, env)?;
        let rhs = self.eval_expr(&b.right, env)?;

        match b.op {
            BinOp::Add(_) => match (lhs, rhs) {
                (Value::Int(a), Value::Int(c)) => Ok(Value::Int(a.wrapping_add(c))),
                (Value::Float(a), Value::Float(c)) => Ok(Value::Float(a + c)),
                (Value::Str(a), Value::Str(c)) => Ok(Value::Str(format!("{a}{c}"))),
                _ => Err(RuntimeError::new("addition impossible entre ces types", b.span()).into()),
            },
            BinOp::Sub(_) => match (lhs, rhs) {
                (Value::Int(a), Value::Int(c)) => Ok(Value::Int(a.wrapping_sub(c))),
                (Value::Float(a), Value::Float(c)) => Ok(Value::Float(a - c)),
                _ => Err(RuntimeError::new("soustraction impossible entre ces types", b.span()).into()),
            },
            BinOp::Mul(_) => match (lhs, rhs) {
                (Value::Int(a), Value::Int(c)) => Ok(Value::Int(a.wrapping_mul(c))),
                (Value::Float(a), Value::Float(c)) => Ok(Value::Float(a * c)),
                _ => Err(RuntimeError::new("multiplication impossible entre ces types", b.span()).into()),
            },
            BinOp::Div(_) => match (lhs, rhs) {
                (Value::Int(_), Value::Int(0)) => Err(RuntimeError::new("division par zéro", b.span()).into()),
                (Value::Int(a), Value::Int(c)) => Ok(Value::Int(a / c)),
                (Value::Float(a), Value::Float(c)) => Ok(Value::Float(a / c)),
                _ => Err(RuntimeError::new("division impossible entre ces types", b.span()).into()),
            },
            BinOp::Rem(_) => match (lhs, rhs) {
                (Value::Int(_), Value::Int(0)) => Err(RuntimeError::new("modulo par zéro", b.span()).into()),
                (Value::Int(a), Value::Int(c)) => Ok(Value::Int(a % c)),
                (Value::Float(a), Value::Float(c)) => Ok(Value::Float(a % c)),
                _ => Err(RuntimeError::new("modulo impossible entre ces types", b.span()).into()),
            },
            BinOp::BitAnd(_) => match (lhs, rhs) {
                (Value::Int(a), Value::Int(c)) => Ok(Value::Int(a & c)),
                (Value::Bool(a), Value::Bool(c)) => Ok(Value::Bool(a & c)),
                _ => Err(RuntimeError::new("opérateur `&` invalide", b.span()).into()),
            },
            BinOp::BitOr(_) => match (lhs, rhs) {
                (Value::Int(a), Value::Int(c)) => Ok(Value::Int(a | c)),
                (Value::Bool(a), Value::Bool(c)) => Ok(Value::Bool(a | c)),
                _ => Err(RuntimeError::new("opérateur `|` invalide", b.span()).into()),
            },
            BinOp::BitXor(_) => match (lhs, rhs) {
                (Value::Int(a), Value::Int(c)) => Ok(Value::Int(a ^ c)),
                (Value::Bool(a), Value::Bool(c)) => Ok(Value::Bool(a ^ c)),
                _ => Err(RuntimeError::new("opérateur `^` invalide", b.span()).into()),
            },
            BinOp::Shl(_) => match (lhs, rhs) {
                (Value::Int(a), Value::Int(c)) => Ok(Value::Int(a.wrapping_shl(c as u32))),
                _ => Err(RuntimeError::new("opérateur `<<` invalide", b.span()).into()),
            },
            BinOp::Shr(_) => match (lhs, rhs) {
                (Value::Int(a), Value::Int(c)) => Ok(Value::Int(a.wrapping_shr(c as u32))),
                _ => Err(RuntimeError::new("opérateur `>>` invalide", b.span()).into()),
            },
            BinOp::Eq(_) => Ok(Value::Bool(values_eq(&lhs, &rhs))),
            BinOp::Ne(_) => Ok(Value::Bool(!values_eq(&lhs, &rhs))),
            BinOp::Lt(_) => eval_cmp(&lhs, &rhs, b.span()).map(|o| Value::Bool(o.is_lt())),
            BinOp::Le(_) => eval_cmp(&lhs, &rhs, b.span()).map(|o| Value::Bool(o.is_le())),
            BinOp::Gt(_) => eval_cmp(&lhs, &rhs, b.span()).map(|o| Value::Bool(o.is_gt())),
            BinOp::Ge(_) => eval_cmp(&lhs, &rhs, b.span()).map(|o| Value::Bool(o.is_ge())),
            _ => Err(RuntimeError::new("opérateur binaire non supporté", b.span()).into()),
        }
    }

    fn eval_assign(
        &mut self,
        left: &Expr,
        right: &Expr,
        compound: Option<CompoundOp>,
        env: &mut Env,
    ) -> Result<Value, ControlFlow> {
        let rhs = self.eval_expr(right, env)?;
        match left {
            Expr::Path(p) => {
                let name = p
                    .path
                    .get_ident()
                    .map(|i| i.to_string())
                    .ok_or_else(|| RuntimeError::new("cible d'assignation invalide", left.span()))?;
                let new_val = match compound {
                    None => rhs,
                    Some(op) => {
                        let cur = env.get(&name).cloned().ok_or_else(|| {
                            RuntimeError::new(format!("variable `{name}` non définie"), left.span())
                        })?;
                        apply_compound(cur, rhs, op, left.span())?
                    }
                };
                if !env.set(&name, new_val) {
                    return Err(RuntimeError::new(format!("variable `{name}` non définie"), left.span()).into());
                }
                Ok(Value::Unit)
            }
            Expr::Index(idx) => {
                let index = match self.eval_expr(&idx.index, env)? {
                    Value::Int(i) if i >= 0 => i as usize,
                    _ => return Err(RuntimeError::new("index invalide", idx.index.span()).into()),
                };
                if let Expr::Path(p) = idx.expr.as_ref() {
                    if let Some(id) = p.path.get_ident() {
                        let name = id.to_string();
                        if let Some(Value::Array(mut items)) = env.get(&name).cloned() {
                            if index >= items.len() {
                                return Err(RuntimeError::new("index hors limites", left.span()).into());
                            }
                            items[index] = match compound {
                                None => rhs,
                                Some(op) => apply_compound(items[index].clone(), rhs, op, left.span())?,
                            };
                            env.set(&name, Value::Array(items));
                            return Ok(Value::Unit);
                        }
                    }
                }
                Err(RuntimeError::new("assignation d'index non supportée", left.span()).into())
            }
            Expr::Field(f) => {
                if let (Expr::Path(p), syn::Member::Named(field_id)) = (f.base.as_ref(), &f.member) {
                    if let Some(var_id) = p.path.get_ident() {
                        let var_name = var_id.to_string();
                        let field_name = field_id.to_string();
                        if let Some(Value::Struct { name, mut fields }) = env.get(&var_name).cloned() {
                            let new_val = match compound {
                                None => rhs,
                                Some(op) => {
                                    let cur = fields.get(&field_name).cloned().unwrap_or(Value::Unit);
                                    apply_compound(cur, rhs, op, left.span())?
                                }
                            };
                            fields.insert(field_name, new_val);
                            env.set(&var_name, Value::Struct { name, fields });
                            return Ok(Value::Unit);
                        }
                    }
                }
                Err(RuntimeError::new("assignation de champ non supportée", left.span()).into())
            }
            _ => Err(RuntimeError::new("cible d'assignation non supportée", left.span()).into()),
        }
    }

    fn eval_if(&mut self, if_expr: &syn::ExprIf, env: &mut Env) -> Result<Value, ControlFlow> {
        if let Expr::Let(let_expr) = if_expr.cond.as_ref() {
            let val = self.eval_expr(&let_expr.expr, env)?;
            env.push();
            let matched = match_pattern(&let_expr.pat, &val, env);
            let res = if matched {
                self.eval_block(&if_expr.then_branch, env)
            } else {
                env.pop();
                if let Some((_, else_branch)) = &if_expr.else_branch {
                    return self.eval_expr(else_branch, env);
                }
                return Ok(Value::Unit);
            };
            env.pop();
            return res;
        }

        let cond = match self.eval_expr(&if_expr.cond, env)? {
            Value::Bool(b) => b,
            other => {
                return Err(RuntimeError::new(
                    format!("condition booléenne attendue, trouvé {}", other.type_name()),
                    if_expr.cond.span(),
                )
                .into());
            }
        };
        if cond {
            self.eval_block(&if_expr.then_branch, env)
        } else if let Some((_, else_branch)) = &if_expr.else_branch {
            self.eval_expr(else_branch, env)
        } else {
            Ok(Value::Unit)
        }
    }

    fn eval_while(&mut self, w: &syn::ExprWhile, env: &mut Env) -> Result<Value, ControlFlow> {
        loop {
            self.tick()?;
            if let Expr::Let(let_expr) = w.cond.as_ref() {
                let val = self.eval_expr(&let_expr.expr, env)?;
                env.push();
                if !match_pattern(&let_expr.pat, &val, env) {
                    env.pop();
                    break;
                }
                let res = self.eval_block(&w.body, env);
                env.pop();
                match res {
                    Ok(_) | Err(ControlFlow::Continue) => continue,
                    Err(ControlFlow::Break(_)) => break,
                    Err(e) => return Err(e),
                }
            } else {
                let cond = match self.eval_expr(&w.cond, env)? {
                    Value::Bool(b) => b,
                    other => {
                        return Err(RuntimeError::new(
                            format!("condition booléenne attendue, trouvé {}", other.type_name()),
                            w.cond.span(),
                        )
                        .into());
                    }
                };
                if !cond {
                    break;
                }
                match self.eval_block(&w.body, env) {
                    Ok(_) | Err(ControlFlow::Continue) => continue,
                    Err(ControlFlow::Break(_)) => break,
                    Err(e) => return Err(e),
                }
            }
        }
        Ok(Value::Unit)
    }

    fn eval_loop(&mut self, l: &syn::ExprLoop, env: &mut Env) -> Result<Value, ControlFlow> {
        loop {
            self.tick()?;
            match self.eval_block(&l.body, env) {
                Ok(_) | Err(ControlFlow::Continue) => continue,
                Err(ControlFlow::Break(val)) => return Ok(val.unwrap_or(Value::Unit)),
                Err(e) => return Err(e),
            }
        }
    }

    fn eval_for(&mut self, fl: &syn::ExprForLoop, env: &mut Env) -> Result<Value, ControlFlow> {
        let iter_val = self.eval_expr(&fl.expr, env)?;
        let items: Vec<Value> = match iter_val {
            Value::Range { start, end, inclusive } => {
                let count = if inclusive {
                    (end - start + 1).max(0)
                } else {
                    (end - start).max(0)
                };
                if count as usize > MAX_STEPS {
                    return Err(ControlFlow::Error(RuntimeError::timeout()));
                }
                if inclusive {
                    (start..=end).map(Value::Int).collect()
                } else {
                    (start..end).map(Value::Int).collect()
                }
            }
            Value::Array(items) => items,
            Value::Str(s) => s.chars().map(Value::Char).collect(),
            other => {
                return Err(RuntimeError::new(
                    format!("impossible d'itérer sur {}", other.type_name()),
                    fl.expr.span(),
                )
                .into());
            }
        };

        for item in items {
            self.tick()?;
            env.push();
            bind_pattern(&fl.pat, item, env)?;
            let res = self.eval_block(&fl.body, env);
            env.pop();
            match res {
                Ok(_) | Err(ControlFlow::Continue) => continue,
                Err(ControlFlow::Break(_)) => break,
                Err(e) => return Err(e),
            }
        }
        Ok(Value::Unit)
    }

    fn eval_match(&mut self, m: &syn::ExprMatch, env: &mut Env) -> Result<Value, ControlFlow> {
        let target = self.eval_expr(&m.expr, env)?;
        for arm in &m.arms {
            env.push();
            if match_pattern(&arm.pat, &target, env) {
                if let Some((_, guard)) = &arm.guard {
                    match self.eval_expr(guard, env)? {
                        Value::Bool(true) => {}
                        _ => {
                            env.pop();
                            continue;
                        }
                    }
                }
                let res = self.eval_expr(&arm.body, env);
                env.pop();
                return res;
            }
            env.pop();
        }
        Err(RuntimeError::new("aucun bras `match` correspondant", m.span()).into())
    }

    fn eval_call(&mut self, c: &syn::ExprCall, env: &mut Env) -> Result<Value, ControlFlow> {
        let mut args = Vec::with_capacity(c.args.len());
        for arg in &c.args {
            args.push(self.eval_expr(arg, env)?);
        }

        if let Expr::Path(p) = c.func.as_ref() {
            let segs: Vec<String> = p.path.segments.iter().map(|s| s.ident.to_string()).collect();
            if segs.len() == 1 {
                let name = &segs[0];
                match name.as_str() {
                    "Some" => {
                        let v = args.into_iter().next().unwrap_or(Value::Unit);
                        return Ok(Value::Option(Some(Box::new(v))));
                    }
                    "Ok" => {
                        let v = args.into_iter().next().unwrap_or(Value::Unit);
                        return Ok(Value::Result(Ok(Box::new(v))));
                    }
                    "Err" => {
                        let v = args.into_iter().next().unwrap_or(Value::Unit);
                        return Ok(Value::Result(Err(Box::new(v))));
                    }
                    _ => {}
                }
                if let Some(Value::Closure { params, body, captured }) = env.get(name).cloned() {
                    return self.invoke_closure(&params, &body, &captured, args, c.span());
                }
                if let Some(enum_name) = self.enum_variants.get(name) {
                    return Ok(Value::Enum {
                        name: enum_name.clone(),
                        variant: name.clone(),
                        payload: args,
                    });
                }
                return self.call_fn(name, args, c.span());
            } else if segs.len() >= 2 {
                let ty = segs[segs.len() - 2].as_str();
                let method = segs[segs.len() - 1].as_str();
                match (ty, method) {
                    ("Vec", "new") | ("Vec", "with_capacity") => return Ok(Value::Array(vec![])),
                    ("HashMap", "new") | ("HashMap", "with_capacity") => {
                        return Ok(Value::Map(HashMap::new()));
                    }
                    ("String", "new") => return Ok(Value::Str(String::new())),
                    ("String", "from") => {
                        let s = args.into_iter().next().map(|v| v.display_str()).unwrap_or_default();
                        return Ok(Value::Str(s));
                    }
                    ("io", "stdin") => return Ok(Value::Stdin),
                    ("io", "stdout") => return Ok(Value::Stdout),
                    ("Command", "new") => {
                        let cmd = args.into_iter().next().map(|v| v.display_str()).unwrap_or_default();
                        return Ok(Value::Command {
                            cmd,
                            args_list: vec![],
                            envs: vec![],
                        });
                    }
                    ("env", "args") => return Ok(Value::Array(vec![])),
                    ("env", "var") => {
                        return Ok(Value::Result(Err(Box::new(Value::Str(
                            "variable d'environnement introuvable".to_string(),
                        )))));
                    }
                    ("fs", "read_to_string") => {
                        return Ok(Value::Result(Err(Box::new(Value::Str(
                            "fs::read_to_string non disponible dans ce sandbox".to_string(),
                        )))));
                    }
                    ("fs", "write") => {
                        return Ok(Value::Result(Ok(Box::new(Value::Unit))));
                    }
                    ("thread", "sleep") => return Ok(Value::Unit),
                    ("Duration", "from_millis") | ("Duration", "from_secs") => {
                        return Ok(Value::Unit);
                    }
                    ("Option", "Some") => {
                        let v = args.into_iter().next().unwrap_or(Value::Unit);
                        return Ok(Value::Option(Some(Box::new(v))));
                    }
                    _ => {}
                }
                if let Some(def) = self.methods.get(ty).and_then(|m| m.get(method)).cloned() {
                    return self.invoke_fn_def(&def, None, args, c.span());
                }
                return Ok(Value::Enum {
                    name: ty.to_string(),
                    variant: method.to_string(),
                    payload: args,
                });
            }
        }
        Err(RuntimeError::new("appel de fonction non supporté", c.span()).into())
    }

    fn invoke_closure(
        &mut self,
        params: &[String],
        body: &Expr,
        captured: &HashMap<String, Value>,
        args: Vec<Value>,
        span: proc_macro2::Span,
    ) -> Result<Value, ControlFlow> {
        if args.len() != params.len() {
            return Err(RuntimeError::new("nombre d'arguments incorrect pour la closure", span).into());
        }
        let mut c_env = Env::new();
        for (k, v) in captured {
            c_env.define(k.clone(), v.clone());
        }
        for (p, v) in params.iter().zip(args) {
            c_env.define(p.clone(), v);
        }
        self.eval_expr(body, &mut c_env)
    }

    fn eval_method_call(&mut self, mc: &syn::ExprMethodCall, env: &mut Env) -> Result<Value, ControlFlow> {
        let method = mc.method.to_string();
        let mut args = Vec::with_capacity(mc.args.len());
        for arg in &mc.args {
            args.push(self.eval_expr(arg, env)?);
        }
        let receiver = self.eval_expr(&mc.receiver, env)?;

        // Check user-defined struct impl methods first.
        if let Value::Struct { ref name, .. } = receiver {
            if let Some(def) = self.methods.get(name).and_then(|m| m.get(&method)).cloned() {
                if def.has_self {
                    return self.invoke_fn_def(&def, Some(receiver), args, mc.span());
                }
            }
        }

        // Mutating methods on variables (`push`, `pop`, `clear`, `reverse`, `sort`, `push_str`).
        if let Expr::Path(p) = mc.receiver.as_ref() {
            if let Some(var_id) = p.path.get_ident() {
                let var_name = var_id.to_string();
                match (&receiver, method.as_str()) {
                    (Value::Array(items), "push") => {
                        if let Some(v) = args.first().cloned() {
                            let mut next = items.clone();
                            next.push(v);
                            env.set(&var_name, Value::Array(next));
                            return Ok(Value::Unit);
                        }
                    }
                    (Value::Array(items), "pop") => {
                        let mut next = items.clone();
                        let popped = next
                            .pop()
                            .map(|v| Value::Option(Some(Box::new(v))))
                            .unwrap_or(Value::Option(None));
                        env.set(&var_name, Value::Array(next));
                        return Ok(popped);
                    }
                    (Value::Array(_), "clear") => {
                        env.set(&var_name, Value::Array(Vec::new()));
                        return Ok(Value::Unit);
                    }
                    (Value::Array(items), "reverse") => {
                        let mut next = items.clone();
                        next.reverse();
                        env.set(&var_name, Value::Array(next));
                        return Ok(Value::Unit);
                    }
                    (Value::Str(s), "push_str") => {
                        if let Some(v) = args.first() {
                            let mut next = s.clone();
                            next.push_str(&v.display_str());
                            env.set(&var_name, Value::Str(next));
                            return Ok(Value::Unit);
                        }
                    }
                    (Value::Str(s), "push") => {
                        if let Some(Value::Char(ch)) = args.first() {
                            let mut next = s.clone();
                            next.push(*ch);
                            env.set(&var_name, Value::Str(next));
                            return Ok(Value::Unit);
                        }
                    }
                    (Value::Map(m), "insert") => {
                        if args.len() >= 2 {
                            let mut next = m.clone();
                            let prev = next.insert(args[0].display_str(), args[1].clone());
                            env.set(&var_name, Value::Map(next));
                            return Ok(Value::Option(prev.map(Box::new)));
                        }
                    }
                    (Value::Map(m), "remove") => {
                        if let Some(k) = args.first() {
                            let mut next = m.clone();
                            let prev = next.remove(&k.display_str());
                            env.set(&var_name, Value::Map(next));
                            return Ok(Value::Option(prev.map(Box::new)));
                        }
                    }
                    (Value::Map(_), "clear") => {
                        env.set(&var_name, Value::Map(HashMap::new()));
                        return Ok(Value::Unit);
                    }
                    _ => {}
                }
            }
        }

        match (receiver, method.as_str()) {
            (v, "clone") => Ok(v),
            (v, "to_string") | (v, "to_owned") => Ok(Value::Str(v.display_str())),
            (Value::Str(s), "len") => Ok(Value::Int(s.len() as i64)),
            (Value::Str(s), "is_empty") => Ok(Value::Bool(s.is_empty())),
            (Value::Str(s), "trim") => Ok(Value::Str(s.trim().to_string())),
            (Value::Str(s), "to_uppercase") => Ok(Value::Str(s.to_uppercase())),
            (Value::Str(s), "to_lowercase") => Ok(Value::Str(s.to_lowercase())),
            (Value::Str(s), "contains") => {
                let needle = args.first().map(Value::display_str).unwrap_or_default();
                Ok(Value::Bool(s.contains(&needle)))
            }
            (Value::Str(s), "starts_with") => {
                let needle = args.first().map(Value::display_str).unwrap_or_default();
                Ok(Value::Bool(s.starts_with(&needle)))
            }
            (Value::Str(s), "ends_with") => {
                let needle = args.first().map(Value::display_str).unwrap_or_default();
                Ok(Value::Bool(s.ends_with(&needle)))
            }
            (Value::Str(s), "replace") => {
                let from = args.first().map(Value::display_str).unwrap_or_default();
                let to = args.get(1).map(Value::display_str).unwrap_or_default();
                Ok(Value::Str(s.replace(&from, &to)))
            }
            (Value::Str(s), "split") => {
                let sep = args.first().map(Value::display_str).unwrap_or_default();
                let parts = s.split(&sep).map(|p| Value::Str(p.to_string())).collect();
                Ok(Value::Array(parts))
            }
            (Value::Str(s), "chars") => Ok(Value::Array(s.chars().map(Value::Char).collect())),
            (Value::Str(s), "bytes") => Ok(Value::Array(s.bytes().map(|b| Value::Int(b as i64)).collect())),
            (Value::Str(s), "lines") => Ok(Value::Array(s.lines().map(|l| Value::Str(l.to_string())).collect())),
            (Value::Str(s), "split_whitespace") => Ok(Value::Array(
                s.split_whitespace().map(|w| Value::Str(w.to_string())).collect(),
            )),
            (Value::Str(s), "repeat") => {
                let n = match args.first() {
                    Some(Value::Int(n)) if *n >= 0 => *n as usize,
                    _ => 1,
                };
                Ok(Value::Str(s.repeat(n)))
            }
            (Value::Str(s), "to_ascii_uppercase") => Ok(Value::Str(s.to_ascii_uppercase())),
            (Value::Str(s), "to_ascii_lowercase") => Ok(Value::Str(s.to_ascii_lowercase())),
            (Value::Char(c), "is_alphabetic") => Ok(Value::Bool(c.is_alphabetic())),
            (Value::Char(c), "is_numeric") | (Value::Char(c), "is_ascii_digit") => {
                Ok(Value::Bool(c.is_ascii_digit()))
            }
            (Value::Char(c), "is_whitespace") => Ok(Value::Bool(c.is_whitespace())),
            (Value::Char(c), "is_uppercase") => Ok(Value::Bool(c.is_uppercase())),
            (Value::Char(c), "is_lowercase") => Ok(Value::Bool(c.is_lowercase())),
            (Value::Char(c), "to_uppercase") => Ok(Value::Str(c.to_uppercase().to_string())),
            (Value::Char(c), "to_lowercase") => Ok(Value::Str(c.to_lowercase().to_string())),
            (Value::Char(c), "to_digit") => {
                let radix = match args.first() {
                    Some(Value::Int(r)) if *r > 0 => *r as u32,
                    _ => 10,
                };
                Ok(Value::Option(c.to_digit(radix).map(|d| Box::new(Value::Int(d as i64)))))
            }
            (Value::Str(s), "parse") => {
                if let Ok(n) = s.trim().parse::<i64>() {
                    Ok(Value::Result(Ok(Box::new(Value::Int(n)))))
                } else if let Ok(f) = s.trim().parse::<f64>() {
                    Ok(Value::Result(Ok(Box::new(Value::Float(f)))))
                } else {
                    Ok(Value::Result(Err(Box::new(Value::Str("parse error".to_string())))))
                }
            }
            (Value::Array(items), "len") => Ok(Value::Int(items.len() as i64)),
            (Value::Array(items), "is_empty") => Ok(Value::Bool(items.is_empty())),
            (Value::Array(items), "get") => {
                let idx = match args.first() {
                    Some(Value::Int(i)) if *i >= 0 => *i as usize,
                    _ => return Ok(Value::Option(None)),
                };
                Ok(Value::Option(items.get(idx).cloned().map(Box::new)))
            }
            (Value::Array(items), "contains") => {
                let found = args.first().is_some_and(|needle| items.iter().any(|x| values_eq(x, needle)));
                Ok(Value::Bool(found))
            }
            (Value::Array(items), "join") => {
                let sep = args.first().map(Value::display_str).unwrap_or_default();
                let joined = items.iter().map(Value::display_str).collect::<Vec<_>>().join(&sep);
                Ok(Value::Str(joined))
            }
            (Value::Array(items), "iter") | (Value::Array(items), "into_iter") | (Value::Array(items), "collect") => {
                Ok(Value::Array(items))
            }
            (Value::Array(items), "enumerate") => {
                let enumerated = items
                    .into_iter()
                    .enumerate()
                    .map(|(i, v)| Value::Tuple(vec![Value::Int(i as i64), v]))
                    .collect();
                Ok(Value::Array(enumerated))
            }
            (Value::Array(mut items), "rev") => {
                items.reverse();
                Ok(Value::Array(items))
            }
            (Value::Array(items), "map") => {
                let Some(Value::Closure { params, body, captured }) = args.into_iter().next() else {
                    return Err(RuntimeError::new("closure attendue dans `.map(...)`", mc.span()).into());
                };
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    out.push(self.invoke_closure(&params, &body, &captured, vec![item], mc.span())?);
                }
                Ok(Value::Array(out))
            }
            (Value::Array(items), "filter") => {
                let Some(Value::Closure { params, body, captured }) = args.into_iter().next() else {
                    return Err(RuntimeError::new("closure attendue dans `.filter(...)`", mc.span()).into());
                };
                let mut out = Vec::new();
                for item in items {
                    if matches!(self.invoke_closure(&params, &body, &captured, vec![item.clone()], mc.span())?, Value::Bool(true)) {
                        out.push(item);
                    }
                }
                Ok(Value::Array(out))
            }
            (Value::Array(items), "sum") => {
                let mut int_sum: i64 = 0;
                let mut float_sum: f64 = 0.0;
                let mut is_float = false;
                for item in items {
                    match item {
                        Value::Int(n) => int_sum += n,
                        Value::Float(f) => {
                            is_float = true;
                            float_sum += f;
                        }
                        _ => {}
                    }
                }
                Ok(if is_float { Value::Float(float_sum + int_sum as f64) } else { Value::Int(int_sum) })
            }
            (Value::Array(items), "any") => {
                let Some(Value::Closure { params, body, captured }) = args.into_iter().next() else {
                    return Err(RuntimeError::new("closure attendue dans `.any(...)`", mc.span()).into());
                };
                for item in items {
                    if matches!(self.invoke_closure(&params, &body, &captured, vec![item], mc.span())?, Value::Bool(true)) {
                        return Ok(Value::Bool(true));
                    }
                }
                Ok(Value::Bool(false))
            }
            (Value::Array(items), "all") => {
                let Some(Value::Closure { params, body, captured }) = args.into_iter().next() else {
                    return Err(RuntimeError::new("closure attendue dans `.all(...)`", mc.span()).into());
                };
                for item in items {
                    if !matches!(self.invoke_closure(&params, &body, &captured, vec![item], mc.span())?, Value::Bool(true)) {
                        return Ok(Value::Bool(false));
                    }
                }
                Ok(Value::Bool(true))
            }
            (Value::Array(items), "find") => {
                let Some(Value::Closure { params, body, captured }) = args.into_iter().next() else {
                    return Err(RuntimeError::new("closure attendue dans `.find(...)`", mc.span()).into());
                };
                for item in items {
                    if matches!(self.invoke_closure(&params, &body, &captured, vec![item.clone()], mc.span())?, Value::Bool(true)) {
                        return Ok(Value::Option(Some(Box::new(item))));
                    }
                }
                Ok(Value::Option(None))
            }
            (Value::Array(items), "position") => {
                let Some(Value::Closure { params, body, captured }) = args.into_iter().next() else {
                    return Err(RuntimeError::new("closure attendue dans `.position(...)`", mc.span()).into());
                };
                for (i, item) in items.into_iter().enumerate() {
                    if matches!(self.invoke_closure(&params, &body, &captured, vec![item], mc.span())?, Value::Bool(true)) {
                        return Ok(Value::Option(Some(Box::new(Value::Int(i as i64)))));
                    }
                }
                Ok(Value::Option(None))
            }
            (Value::Array(items), "fold") => {
                let mut acc = args.first().cloned().unwrap_or(Value::Unit);
                let Some(Value::Closure { params, body, captured }) = args.into_iter().nth(1) else {
                    return Err(RuntimeError::new("closure attendue dans `.fold(...)`", mc.span()).into());
                };
                for item in items {
                    acc = self.invoke_closure(&params, &body, &captured, vec![acc, item], mc.span())?;
                }
                Ok(acc)
            }
            (Value::Array(items), "flat_map") => {
                let Some(Value::Closure { params, body, captured }) = args.into_iter().next() else {
                    return Err(RuntimeError::new("closure attendue dans `.flat_map(...)`", mc.span()).into());
                };
                let mut out = Vec::new();
                for item in items {
                    match self.invoke_closure(&params, &body, &captured, vec![item], mc.span())? {
                        Value::Array(sub) => out.extend(sub),
                        other => out.push(other),
                    }
                }
                Ok(Value::Array(out))
            }
            (Value::Array(items), "zip") => {
                let other = match args.into_iter().next() {
                    Some(Value::Array(v)) => v,
                    _ => vec![],
                };
                let zipped = items
                    .into_iter()
                    .zip(other)
                    .map(|(a, b)| Value::Tuple(vec![a, b]))
                    .collect();
                Ok(Value::Array(zipped))
            }
            (Value::Array(items), "skip") => {
                let n = match args.first() {
                    Some(Value::Int(n)) if *n >= 0 => *n as usize,
                    _ => 0,
                };
                Ok(Value::Array(items.into_iter().skip(n).collect()))
            }
            (Value::Array(items), "take") => {
                let n = match args.first() {
                    Some(Value::Int(n)) if *n >= 0 => *n as usize,
                    _ => 0,
                };
                Ok(Value::Array(items.into_iter().take(n).collect()))
            }
            (Value::Array(items), "chain") => {
                let other = match args.into_iter().next() {
                    Some(Value::Array(v)) => v,
                    _ => vec![],
                };
                let mut result = items;
                result.extend(other);
                Ok(Value::Array(result))
            }
            (Value::Array(items), "count") => Ok(Value::Int(items.len() as i64)),
            (Value::Array(items), "last") => Ok(Value::Option(items.into_iter().last().map(Box::new))),
            (Value::Array(items), "first") => Ok(Value::Option(items.into_iter().next().map(Box::new))),
            (Value::Array(items), "max") => {
                let result = items.into_iter().reduce(|a, b| match (&a, &b) {
                    (Value::Int(x), Value::Int(y)) => if x >= y { a } else { b },
                    (Value::Float(x), Value::Float(y)) => if x >= y { a } else { b },
                    _ => a,
                });
                Ok(Value::Option(result.map(Box::new)))
            }
            (Value::Array(items), "min") => {
                let result = items.into_iter().reduce(|a, b| match (&a, &b) {
                    (Value::Int(x), Value::Int(y)) => if x <= y { a } else { b },
                    (Value::Float(x), Value::Float(y)) => if x <= y { a } else { b },
                    _ => a,
                });
                Ok(Value::Option(result.map(Box::new)))
            }
            (Value::Array(items), "product") => {
                let mut int_prod: i64 = 1;
                let mut float_prod: f64 = 1.0;
                let mut is_float = false;
                for item in items {
                    match item {
                        Value::Int(n) => int_prod *= n,
                        Value::Float(f) => {
                            is_float = true;
                            float_prod *= f;
                        }
                        _ => {}
                    }
                }
                Ok(if is_float { Value::Float(float_prod * int_prod as f64) } else { Value::Int(int_prod) })
            }
            (Value::Array(items), "windows") => {
                let n = match args.first() {
                    Some(Value::Int(n)) if *n > 0 => *n as usize,
                    _ => 1,
                };
                let windows: Vec<Value> = items.windows(n).map(|w| Value::Array(w.to_vec())).collect();
                Ok(Value::Array(windows))
            }
            (Value::Array(items), "chunks") | (Value::Array(items), "chunks_exact") => {
                let n = match args.first() {
                    Some(Value::Int(n)) if *n > 0 => *n as usize,
                    _ => 1,
                };
                let chunks: Vec<Value> = items.chunks(n).map(|c| Value::Array(c.to_vec())).collect();
                Ok(Value::Array(chunks))
            }
            (Value::Range { start, end, inclusive }, "rev") => {
                let items: Vec<Value> = if inclusive {
                    (start..=end).rev().map(Value::Int).collect()
                } else {
                    (start..end).rev().map(Value::Int).collect()
                };
                Ok(Value::Array(items))
            }
            (Value::Range { start, end, inclusive }, "collect") => {
                let items: Vec<Value> = if inclusive {
                    (start..=end).map(Value::Int).collect()
                } else {
                    (start..end).map(Value::Int).collect()
                };
                Ok(Value::Array(items))
            }
            (Value::Range { start, end, inclusive }, "sum") => {
                let s: i64 = if inclusive { (start..=end).sum() } else { (start..end).sum() };
                Ok(Value::Int(s))
            }
            (Value::Int(n), "abs") => Ok(Value::Int(n.abs())),
            (Value::Int(n), "pow") => {
                let exp = match args.first() {
                    Some(Value::Int(e)) if *e >= 0 => *e as u32,
                    _ => return Err(RuntimeError::new("exposant positif attendu", mc.span()).into()),
                };
                Ok(Value::Int(n.wrapping_pow(exp)))
            }
            (Value::Int(n), "min") => match args.first() {
                Some(Value::Int(m)) => Ok(Value::Int(n.min(*m))),
                _ => Err(RuntimeError::new("entier attendu", mc.span()).into()),
            },
            (Value::Int(n), "max") => match args.first() {
                Some(Value::Int(m)) => Ok(Value::Int(n.max(*m))),
                _ => Err(RuntimeError::new("entier attendu", mc.span()).into()),
            },
            (Value::Float(f), "abs") => Ok(Value::Float(f.abs())),
            (Value::Float(f), "sqrt") => Ok(Value::Float(f.sqrt())),
            (Value::Float(f), "floor") => Ok(Value::Float(f.floor())),
            (Value::Float(f), "ceil") => Ok(Value::Float(f.ceil())),
            (Value::Float(f), "round") => Ok(Value::Float(f.round())),
            (Value::Option(Some(v)), "unwrap") => Ok(*v),
            (Value::Option(None), "unwrap") => Err(RuntimeError::new("panique: `Option::unwrap()` sur `None`", mc.span()).into()),
            (Value::Option(Some(v)), "expect") => Ok(*v),
            (Value::Option(None), "expect") => {
                let msg = args.first().map(Value::display_str).unwrap_or_else(|| "Option::expect() sur None".to_string());
                Err(RuntimeError::new(msg, mc.span()).into())
            }
            (Value::Option(Some(v)), "unwrap_or") => Ok(*v),
            (Value::Option(None), "unwrap_or") => Ok(args.into_iter().next().unwrap_or(Value::Unit)),
            (Value::Option(opt), "is_some") => Ok(Value::Bool(opt.is_some())),
            (Value::Option(opt), "is_none") => Ok(Value::Bool(opt.is_none())),
            (Value::Result(Ok(v)), "unwrap") => Ok(*v),
            (Value::Result(Err(e)), "unwrap") => Err(RuntimeError::new(format!("panique: `Result::unwrap()` sur `Err({})`", e.debug_str()), mc.span()).into()),
            (Value::Result(Ok(v)), "expect") => Ok(*v),
            (Value::Result(Err(e)), "expect") => {
                let msg = args.first().map(Value::display_str).unwrap_or_else(|| "Result::expect()".to_string());
                Err(RuntimeError::new(format!("{msg}: {}", e.debug_str()), mc.span()).into())
            }
            (Value::Result(Ok(v)), "unwrap_or") => Ok(*v),
            (Value::Result(Err(_)), "unwrap_or") => Ok(args.into_iter().next().unwrap_or(Value::Unit)),
            (Value::Result(r), "is_ok") => Ok(Value::Bool(r.is_ok())),
            (Value::Result(r), "is_err") => Ok(Value::Bool(r.is_err())),
            (Value::Stdin, "read_line") => {
                if self.stdin_lines.is_empty() {
                    self.needs_input = true;
                    return Err(ControlFlow::NeedsInput);
                }
                let line = self.stdin_lines.pop_front().unwrap_or_default();
                let bytes = line.len() as i64;
                if let Some(arg_expr) = mc.args.first() {
                    if let Some(var_name) = extract_mut_var_name(arg_expr) {
                        let cur = match env.get(&var_name) {
                            Some(Value::Str(s)) => s.clone(),
                            _ => String::new(),
                        };
                        env.set(&var_name, Value::Str(format!("{cur}{line}")));
                    }
                }
                Ok(Value::Result(Ok(Box::new(Value::Int(bytes)))))
            }
            (Value::Stdout, "flush") => Ok(Value::Result(Ok(Box::new(Value::Unit)))),
            (Value::Command { cmd, mut args_list, envs }, "arg") => {
                let a = args.into_iter().next().map(|v| v.display_str()).unwrap_or_default();
                args_list.push(a);
                Ok(Value::Command { cmd, args_list, envs })
            }
            (Value::Command { cmd, mut args_list, envs }, "args") => {
                if let Some(Value::Array(items)) = args.into_iter().next() {
                    for item in items {
                        args_list.push(item.display_str());
                    }
                }
                Ok(Value::Command { cmd, args_list, envs })
            }
            (Value::Command { cmd, args_list, mut envs }, "env") => {
                let k = args.first().map(Value::display_str).unwrap_or_default();
                let v = args.get(1).map(Value::display_str).unwrap_or_default();
                envs.push((k, v));
                Ok(Value::Command { cmd, args_list, envs })
            }
            (Value::Command { cmd, args_list, envs }, "current_dir") => {
                Ok(Value::Command { cmd, args_list, envs })
            }
            (Value::Command { cmd, .. }, "output")
            | (Value::Command { cmd, .. }, "status")
            | (Value::Command { cmd, .. }, "spawn") => {
                let msg = format!("Commande `{cmd}` non exécutable sur Android (sandbox)");
                Ok(Value::Result(Err(Box::new(Value::Str(msg)))))
            }
            (Value::Map(m), "len") => Ok(Value::Int(m.len() as i64)),
            (Value::Map(m), "is_empty") => Ok(Value::Bool(m.is_empty())),
            (Value::Map(m), "get") => {
                let key = args.first().map(Value::display_str).unwrap_or_default();
                Ok(Value::Option(m.get(&key).cloned().map(Box::new)))
            }
            (Value::Map(m), "contains_key") => {
                let key = args.first().map(Value::display_str).unwrap_or_default();
                Ok(Value::Bool(m.contains_key(&key)))
            }
            (other, m) => Err(RuntimeError::new(
                format!("méthode `{m}` non supportée sur `{}`", other.type_name()),
                mc.span(),
            )
            .into()),
        }
    }

    fn eval_macro(
        &mut self,
        name: &str,
        tokens: &TokenStream,
        span: proc_macro2::Span,
        env: &mut Env,
    ) -> Result<Value, ControlFlow> {
        let args: syn::punctuated::Punctuated<Expr, syn::Token![,]> =
            syn::parse::Parser::parse2(syn::punctuated::Punctuated::parse_terminated, tokens.clone())
                .map_err(|e| RuntimeError::new(format!("macro `{name}!` invalide : {e}"), span))?;
        let arg_vec: Vec<Expr> = args.into_iter().collect();

        match name {
            "println" | "print" | "eprintln" | "eprint" | "format" => {
                let formatted = if arg_vec.is_empty() {
                    String::new()
                } else {
                    let fmt_str = match self.eval_expr(&arg_vec[0], env)? {
                        Value::Str(s) => s,
                        other => other.display_str(),
                    };
                    let mut vals = Vec::with_capacity(arg_vec.len().saturating_sub(1));
                    for e in &arg_vec[1..] {
                        vals.push(self.eval_expr(e, env)?);
                    }
                    format_string(&fmt_str, &vals, env, span)?
                };
                match name {
                    "println" => {
                        self.write_stdout(&format!("{formatted}\n"));
                        Ok(Value::Unit)
                    }
                    "print" => {
                        self.write_stdout(&formatted);
                        Ok(Value::Unit)
                    }
                    "eprintln" => {
                        self.write_stderr(&format!("{formatted}\n"));
                        Ok(Value::Unit)
                    }
                    "eprint" => {
                        self.write_stderr(&formatted);
                        Ok(Value::Unit)
                    }
                    _ => Ok(Value::Str(formatted)),
                }
            }
            "vec" => {
                // Handle vec![val; count] or vec![a, b, c]
                if let Ok(repeat) = syn::parse2::<syn::ExprRepeat>(tokens.clone()) {
                    let val = self.eval_expr(&repeat.expr, env)?;
                    let count = match self.eval_expr(&repeat.len, env)? {
                        Value::Int(n) if n >= 0 => n as usize,
                        _ => return Err(RuntimeError::new("taille invalide dans `vec!`", span).into()),
                    };
                    return Ok(Value::Array(vec![val; count.min(100_000)]));
                }
                let mut items = Vec::with_capacity(arg_vec.len());
                for e in &arg_vec {
                    items.push(self.eval_expr(e, env)?);
                }
                Ok(Value::Array(items))
            }
            "dbg" => {
                let mut last = Value::Unit;
                for e in &arg_vec {
                    let v = self.eval_expr(e, env)?;
                    let line = e.span().start().line;
                    self.write_stderr(&format!("[ligne {line}] = {}\n", v.debug_str()));
                    last = v;
                }
                Ok(last)
            }
            "assert" => {
                let cond = arg_vec
                    .first()
                    .map(|e| self.eval_expr(e, env))
                    .transpose()?
                    .unwrap_or(Value::Bool(false));
                if !matches!(cond, Value::Bool(true)) {
                    return Err(RuntimeError::new("assertion échouée (`assert!`)", span).into());
                }
                Ok(Value::Unit)
            }
            "assert_eq" => {
                if arg_vec.len() < 2 {
                    return Err(RuntimeError::new("`assert_eq!` requiert 2 arguments", span).into());
                }
                let l = self.eval_expr(&arg_vec[0], env)?;
                let r = self.eval_expr(&arg_vec[1], env)?;
                if !values_eq(&l, &r) {
                    return Err(RuntimeError::new(
                        format!("`assert_eq!` échoué : {} != {}", l.debug_str(), r.debug_str()),
                        span,
                    )
                    .into());
                }
                Ok(Value::Unit)
            }
            "assert_ne" => {
                if arg_vec.len() < 2 {
                    return Err(RuntimeError::new("`assert_ne!` requiert 2 arguments", span).into());
                }
                let l = self.eval_expr(&arg_vec[0], env)?;
                let r = self.eval_expr(&arg_vec[1], env)?;
                if values_eq(&l, &r) {
                    return Err(RuntimeError::new(
                        format!("`assert_ne!` échoué : les valeurs sont égales ({})", l.debug_str()),
                        span,
                    )
                    .into());
                }
                Ok(Value::Unit)
            }
            "panic" | "todo" | "unimplemented" => {
                let msg = if let Some(first) = arg_vec.first() {
                    self.eval_expr(first, env)?.display_str()
                } else {
                    format!("panique (`{name}!`)")
                };
                Err(RuntimeError::new(msg, span).into())
            }
            _ => Err(RuntimeError::new(format!("macro `{name}!` non supportée"), span).into()),
        }
    }
}

#[derive(Copy, Clone)]
enum CompoundOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

fn apply_compound(cur: Value, rhs: Value, op: CompoundOp, span: proc_macro2::Span) -> Result<Value, ControlFlow> {
    match (cur, rhs, op) {
        (Value::Int(a), Value::Int(b), CompoundOp::Add) => Ok(Value::Int(a.wrapping_add(b))),
        (Value::Int(a), Value::Int(b), CompoundOp::Sub) => Ok(Value::Int(a.wrapping_sub(b))),
        (Value::Int(a), Value::Int(b), CompoundOp::Mul) => Ok(Value::Int(a.wrapping_mul(b))),
        (Value::Int(_), Value::Int(0), CompoundOp::Div | CompoundOp::Rem) => {
            Err(RuntimeError::new("division par zéro", span).into())
        }
        (Value::Int(a), Value::Int(b), CompoundOp::Div) => Ok(Value::Int(a / b)),
        (Value::Int(a), Value::Int(b), CompoundOp::Rem) => Ok(Value::Int(a % b)),
        (Value::Float(a), Value::Float(b), CompoundOp::Add) => Ok(Value::Float(a + b)),
        (Value::Float(a), Value::Float(b), CompoundOp::Sub) => Ok(Value::Float(a - b)),
        (Value::Float(a), Value::Float(b), CompoundOp::Mul) => Ok(Value::Float(a * b)),
        (Value::Float(a), Value::Float(b), CompoundOp::Div) => Ok(Value::Float(a / b)),
        (Value::Float(a), Value::Float(b), CompoundOp::Rem) => Ok(Value::Float(a % b)),
        _ => Err(RuntimeError::new("opération composée invalide", span).into()),
    }
}

fn format_string(
    fmt_str: &str,
    vals: &[Value],
    env: &Env,
    span: proc_macro2::Span,
) -> Result<String, ControlFlow> {
    let mut out = String::with_capacity(fmt_str.len());
    let chars: Vec<char> = fmt_str.chars().collect();
    let mut i = 0;
    let mut arg_idx = 0;

    while i < chars.len() {
        if chars[i] == '{' {
            if i + 1 < chars.len() && chars[i + 1] == '{' {
                out.push('{');
                i += 2;
                continue;
            }
            if let Some(close_rel) = chars[i + 1..].iter().position(|&c| c == '}') {
                let spec: String = chars[i + 1..i + 1 + close_rel].iter().collect();
                let (name_part, is_debug) = if let Some((left, right)) = spec.split_once(':') {
                    (left.trim(), right.contains('?'))
                } else {
                    (spec.trim(), false)
                };
                let val = if name_part.is_empty() {
                    let v = vals.get(arg_idx).ok_or_else(|| {
                        RuntimeError::new("argument de formatage manquant", span)
                    })?;
                    arg_idx += 1;
                    v
                } else if let Ok(pos) = name_part.parse::<usize>() {
                    vals.get(pos).ok_or_else(|| {
                        RuntimeError::new("index de formatage manquant", span)
                    })?
                } else {
                    env.get(name_part).ok_or_else(|| {
                        RuntimeError::new(format!("variable `{name_part}` non définie"), span)
                    })?
                };
                out.push_str(&if is_debug { val.debug_str() } else { val.display_str() });
                i = i + 2 + close_rel;
                continue;
            }
        } else if chars[i] == '}' && i + 1 < chars.len() && chars[i + 1] == '}' {
            out.push('}');
            i += 2;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }

    Ok(out)
}

fn parse_sig_params(sig: &syn::Signature) -> (Vec<String>, bool) {
    let mut params = Vec::new();
    let mut has_self = false;
    for arg in &sig.inputs {
        match arg {
            syn::FnArg::Receiver(_) => has_self = true,
            syn::FnArg::Typed(pt) => {
                params.push(pat_ident_name(&pt.pat).unwrap_or_else(|| "_".to_string()));
            }
        }
    }
    (params, has_self)
}

fn parse_fn_def(f: &syn::ItemFn) -> FnDef {
    let (params, has_self) = parse_sig_params(&f.sig);
    FnDef {
        params,
        has_self,
        body: (*f.block).clone(),
    }
}

fn pat_ident_name(pat: &Pat) -> Option<String> {
    match pat {
        Pat::Ident(pi) => Some(pi.ident.to_string()),
        Pat::Type(pt) => pat_ident_name(&pt.pat),
        Pat::Reference(pr) => pat_ident_name(&pr.pat),
        _ => None,
    }
}

fn bind_pattern(pat: &Pat, val: Value, env: &mut Env) -> Result<(), ControlFlow> {
    match pat {
        Pat::Wild(_) => Ok(()),
        Pat::Ident(pi) => {
            env.define(pi.ident.to_string(), val);
            Ok(())
        }
        Pat::Type(pt) => bind_pattern(&pt.pat, val, env),
        Pat::Reference(pr) => bind_pattern(&pr.pat, val, env),
        Pat::Tuple(pt) => {
            if let Value::Tuple(items) = val {
                for (sub_pat, sub_val) in pt.elems.iter().zip(items) {
                    bind_pattern(sub_pat, sub_val, env)?;
                }
                Ok(())
            } else {
                Err(RuntimeError::new("tuple attendu pour la déstructuration", pat.span()).into())
            }
        }
        _ => Ok(()),
    }
}

fn match_pattern(pat: &Pat, val: &Value, env: &mut Env) -> bool {
    match pat {
        Pat::Wild(_) => true,
        Pat::Ident(pi) => {
            let name = pi.ident.to_string();
            if name == "None" {
                return matches!(val, Value::Option(None));
            }
            env.define(name, val.clone());
            true
        }
        Pat::Lit(pl) => eval_lit(&pl.lit, pl.span())
            .map(|lv| values_eq(&lv, val))
            .unwrap_or(false),
        Pat::Range(pr) => {
            let start = pr.start.as_ref().and_then(|e| match e.as_ref() {
                Expr::Lit(ExprLit { lit: Lit::Int(li), .. }) => li.base10_parse::<i64>().ok(),
                _ => None,
            });
            let end = pr.end.as_ref().and_then(|e| match e.as_ref() {
                Expr::Lit(ExprLit { lit: Lit::Int(li), .. }) => li.base10_parse::<i64>().ok(),
                _ => None,
            });
            if let Value::Int(n) = val {
                let ok_start = start.is_none_or(|s| *n >= s);
                let ok_end = end.is_none_or(|e| {
                    if matches!(pr.limits, syn::RangeLimits::Closed(_)) {
                        *n <= e
                    } else {
                        *n < e
                    }
                });
                ok_start && ok_end
            } else {
                false
            }
        }
        Pat::Or(po) => po.cases.iter().any(|c| match_pattern(c, val, env)),
        Pat::Tuple(pt) => {
            if let Value::Tuple(items) = val {
                pt.elems.len() == items.len()
                    && pt.elems.iter().zip(items).all(|(p, v)| match_pattern(p, v, env))
            } else {
                false
            }
        }
        Pat::TupleStruct(pts) => {
            let name = pts.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
            match (name.as_str(), val) {
                ("Some", Value::Option(Some(inner))) => {
                    pts.elems.first().is_none_or(|p| match_pattern(p, inner, env))
                }
                ("Ok", Value::Result(Ok(inner))) => {
                    pts.elems.first().is_none_or(|p| match_pattern(p, inner, env))
                }
                ("Err", Value::Result(Err(inner))) => {
                    pts.elems.first().is_none_or(|p| match_pattern(p, inner, env))
                }
                (variant, Value::Enum { variant: ev, payload, .. }) if variant == ev => {
                    pts.elems.len() == payload.len()
                        && pts.elems.iter().zip(payload).all(|(p, v)| match_pattern(p, v, env))
                }
                _ => false,
            }
        }
        Pat::Path(pp) => {
            let segs: Vec<String> = pp.path.segments.iter().map(|s| s.ident.to_string()).collect();
            let last = segs.last().map(String::as_str).unwrap_or("");
            match (last, val) {
                ("None", Value::Option(None)) => true,
                (v, Value::Enum { variant, payload, .. }) => v == variant && payload.is_empty(),
                _ => false,
            }
        }
        Pat::Paren(p) => match_pattern(&p.pat, val, env),
        _ => false,
    }
}

fn eval_lit(lit: &Lit, span: proc_macro2::Span) -> Result<Value, ControlFlow> {
    match lit {
        Lit::Int(li) => li
            .base10_parse::<i64>()
            .map(Value::Int)
            .map_err(|e| RuntimeError::new(format!("entier invalide : {e}"), span).into()),
        Lit::Float(lf) => lf
            .base10_parse::<f64>()
            .map(Value::Float)
            .map_err(|e| RuntimeError::new(format!("flottant invalide : {e}"), span).into()),
        Lit::Bool(lb) => Ok(Value::Bool(lb.value)),
        Lit::Char(lc) => Ok(Value::Char(lc.value())),
        Lit::Str(ls) => Ok(Value::Str(ls.value())),
        _ => Err(RuntimeError::new("littéral non supporté", span).into()),
    }
}

fn eval_cast(val: Value, ty: &syn::Type, span: proc_macro2::Span) -> Result<Value, ControlFlow> {
    let ty_name = match ty {
        syn::Type::Path(tp) => tp.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default(),
        _ => String::new(),
    };
    match (val, ty_name.as_str()) {
        (Value::Int(n), "f64" | "f32") => Ok(Value::Float(n as f64)),
        (Value::Float(f), "i64" | "i32" | "isize" | "usize" | "u32" | "u64") => Ok(Value::Int(f as i64)),
        (Value::Int(n), "i64" | "i32" | "isize" | "usize" | "u32" | "u64" | "u8" | "i8") => Ok(Value::Int(n)),
        (Value::Int(n), "char") => Ok(Value::Char(char::from_u32(n as u32).unwrap_or('\0'))),
        (Value::Char(c), "u8" | "u32" | "i32" | "i64" | "usize") => Ok(Value::Int(c as i64)),
        (v, _) => Err(RuntimeError::new(format!("conversion `as {ty_name}` non supportée pour {}", v.type_name()), span).into()),
    }
}

fn values_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Char(x), Value::Char(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Unit, Value::Unit) => true,
        (Value::Tuple(x), Value::Tuple(y)) | (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(l, r)| values_eq(l, r))
        }
        (Value::Option(None), Value::Option(None)) => true,
        (Value::Option(Some(x)), Value::Option(Some(y))) => values_eq(x, y),
        (Value::Result(Ok(x)), Value::Result(Ok(y))) => values_eq(x, y),
        (Value::Result(Err(x)), Value::Result(Err(y))) => values_eq(x, y),
        (Value::Enum { name: n1, variant: v1, payload: p1 }, Value::Enum { name: n2, variant: v2, payload: p2 }) => {
            n1 == n2 && v1 == v2 && p1.len() == p2.len() && p1.iter().zip(p2).all(|(l, r)| values_eq(l, r))
        }
        _ => false,
    }
}

fn eval_cmp(a: &Value, b: &Value, span: proc_macro2::Span) -> Result<std::cmp::Ordering, ControlFlow> {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Ok(x.cmp(y)),
        (Value::Float(x), Value::Float(y)) => x
            .partial_cmp(y)
            .ok_or_else(|| RuntimeError::new("comparaison NaN impossible", span).into()),
        (Value::Char(x), Value::Char(y)) => Ok(x.cmp(y)),
        (Value::Str(x), Value::Str(y)) => Ok(x.cmp(y)),
        _ => Err(RuntimeError::new("comparaison impossible entre ces types", span).into()),
    }
}

fn extract_mut_var_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Reference(r) => extract_mut_var_name(&r.expr),
        Expr::Paren(p) => extract_mut_var_name(&p.expr),
        Expr::Path(p) => p.path.get_ident().map(|id| id.to_string()),
        _ => None,
    }
}

pub fn run_rust(code: &str) -> RunResult {
    run_rust_with_stdin(code, "")
}

pub fn run_rust_with_stdin(code: &str, stdin: &str) -> RunResult {
    let (compile, parsed) = check_rust(code);
    let Some(file) = parsed else {
        return RunResult {
            success: false,
            timed_out: false,
            needs_input: false,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: compile.duration_ms,
            truncated: false,
            compile,
        };
    };

    let started = Instant::now();
    let mut interp = Interpreter::new(stdin);
    let exec_res = interp
        .load_file(&file)
        .and_then(|_| interp.call_fn("main", vec![], proc_macro2::Span::call_site()));
    let duration_ms = started.elapsed().as_millis() as u64;

    match exec_res {
        Ok(_) => RunResult {
            success: true,
            timed_out: false,
            needs_input: false,
            exit_code: Some(0),
            stdout: interp.stdout,
            stderr: interp.stderr,
            duration_ms,
            truncated: interp.truncated,
            compile,
        },
        Err(ControlFlow::NeedsInput) => RunResult {
            success: true,
            timed_out: false,
            needs_input: true,
            exit_code: None,
            stdout: interp.stdout,
            stderr: interp.stderr,
            duration_ms,
            truncated: interp.truncated,
            compile,
        },
        Err(ControlFlow::Error(err)) => {
            let line = err.line;
            let col = err.column;
            let rendered = if let (Some(l), Some(c)) = (line, col) {
                render_error(code, l, c, &err.message)
            } else {
                format!("error: {}", err.message)
            };
            if !interp.stderr.is_empty() && !interp.stderr.ends_with('\n') {
                interp.stderr.push('\n');
            }
            interp.stderr.push_str(&rendered);
            let mut compile_with_err = compile;
            compile_with_err.diagnostics.push(Diagnostic {
                level: "error".to_string(),
                message: err.message,
                line,
                column: col,
                rendered: Some(rendered),
            });
            RunResult {
                success: false,
                timed_out: err.timed_out,
                needs_input: false,
                exit_code: Some(1),
                stdout: interp.stdout,
                stderr: interp.stderr,
                duration_ms,
                truncated: interp.truncated,
                compile: compile_with_err,
            }
        }
        Err(_) => RunResult {
            success: true,
            timed_out: false,
            needs_input: false,
            exit_code: Some(0),
            stdout: interp.stdout,
            stderr: interp.stderr,
            duration_ms,
            truncated: interp.truncated,
            compile,
        },
    }
}

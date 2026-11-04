//! Les expressions que l'on saisit pour regarder ou modifier une variable : `x`, `shape.corner.x`, `*p`, `&n`, `a[2][3]`, `n + 1`,
//! `(char)n`, `sizeof(Shape)`, `x = 5`, `i++`, `n > 2 ? a : b`, `factorial(4)`, `$r0 = 1`. La syntaxe est celle du C.

use thiserror::Error;

use super::eval::{CallArg, Context, Place, Value, ValueKind, locate, read_value};
use super::{DebugInfo, Encoding, TypeId, TypeKind};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{0}")]
pub struct EvalError(pub String);

fn fail<T>(message: impl Into<String>) -> Result<T, EvalError> {
    Err(EvalError(message.into()))
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(i64),
    Float(f64),
    Ident(String),
    Punct(&'static str),
}

/// Les opérateurs, les plus longs d'abord.
const PUNCT: &[&str] = &[
    "<<=", ">>=", "::", "->", "++", "--", "<<", ">>", "<=", ">=", "==", "!=", "&&", "||", "+=",
    "-=", "*=", "/=", "%=", "&=", "|=", "^=", ".", "[", "]", "(", ")", "*", "&", "+", "-", "/",
    "%", "<", ">", "!", "~", ",", "?", ":", "=", "|", "^",
];

fn escape(c: char) -> Option<i64> {
    Some(match c {
        'n' => 10,
        't' => 9,
        'r' => 13,
        '0' => 0,
        '\\' => 92,
        '\'' => 39,
        '"' => 34,
        _ => return None,
    })
}

fn lex(input: &str) -> Result<Vec<Token>, EvalError> {
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() {
            let start = i;
            if c == '0' && matches!(chars.get(i + 1), Some('x' | 'X')) {
                i += 2;
                while i < chars.len() && chars[i].is_ascii_hexdigit() {
                    i += 1;
                }
                let s: String = chars[start + 2..i].iter().collect();
                out.push(Token::Number(
                    i64::from_str_radix(&s, 16).map_err(|_| EvalError("nombre invalide".into()))?,
                ));
            } else {
                let mut is_float = false;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    is_float |= chars[i] == '.';
                    i += 1;
                }
                if matches!(chars.get(i), Some('e' | 'E')) {
                    let mut j = i + 1;
                    if matches!(chars.get(j), Some('+' | '-')) {
                        j += 1;
                    }
                    if chars.get(j).is_some_and(char::is_ascii_digit) {
                        is_float = true;
                        i = j;
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                    }
                }
                let s: String = chars[start..i].iter().collect();
                // Un suffixe (`u`, `l`, `f`) ne change pas la valeur.
                while matches!(chars.get(i), Some('u' | 'U' | 'l' | 'L' | 'f' | 'F')) {
                    is_float |= matches!(chars[i], 'f' | 'F') && !s.starts_with("0x");
                    i += 1;
                }
                if is_float {
                    out.push(Token::Float(
                        s.parse().map_err(|_| EvalError("nombre invalide".into()))?,
                    ));
                } else {
                    out.push(Token::Number(
                        s.parse().map_err(|_| EvalError("nombre invalide".into()))?,
                    ));
                }
            }
        } else if c == '\'' {
            // Un caractère : 'a', '\n', '\x41'.
            let (value, used) = match (chars.get(i + 1), chars.get(i + 2)) {
                (Some('\\'), Some('x')) => {
                    let hex: String = chars[i + 3..]
                        .iter()
                        .take_while(|c| c.is_ascii_hexdigit())
                        .collect();
                    (
                        i64::from_str_radix(&hex, 16)
                            .map_err(|_| EvalError("caractère invalide".into()))?,
                        3 + hex.len(),
                    )
                }
                (Some('\\'), Some(e)) => (
                    escape(*e).ok_or_else(|| EvalError("échappement inconnu".into()))?,
                    3,
                ),
                (Some(ch), _) => (*ch as i64, 2),
                _ => return fail("caractère incomplet"),
            };
            if chars.get(i + used) != Some(&'\'') {
                return fail("« ' » attendue après le caractère");
            }
            out.push(Token::Number(value));
            i += used + 1;
        } else if c.is_alphabetic() || c == '_' || c == '$' {
            let start = i;
            while i < chars.len()
                && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '$')
            {
                i += 1;
            }
            out.push(Token::Ident(chars[start..i].iter().collect()));
        } else {
            let rest: String = chars[i..].iter().take(3).collect();
            match PUNCT.iter().find(|p| rest.starts_with(**p)) {
                Some(p) => {
                    out.push(Token::Punct(p));
                    i += p.len();
                }
                None => return fail(format!("caractère inattendu : {c}")),
            }
        }
    }
    Ok(out)
}

/// Un nombre, entier ou à virgule.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Num {
    I(i64),
    F(f64),
}

/// Une valeur en cours de calcul.
#[derive(Debug, Clone)]
enum Operand {
    Int(i64),
    Float(f64),
    /// Un nombre de type connu (résultat d'une conversion ou d'un appel).
    Typed {
        ty: TypeId,
        num: Num,
    },
    /// Un objet du programme, en mémoire ou dans des registres.
    Object {
        ty: Option<TypeId>,
        place: Place,
    },
    /// Un pointeur calculé (`&x`, `(int *)p`), avec le type pointé.
    Address {
        target: Option<TypeId>,
        address: u32,
    },
    /// Une fonction : son adresse et le type qu'elle rend.
    Func {
        address: u32,
        ret: Option<TypeId>,
    },
    /// Un registre du processeur (`$r0`).
    Register(usize),
    /// Plusieurs fonctions de même nom (surcharges C++) : leurs adresses ; l'appel choisit d'après les arguments.
    Overloads(Vec<u32>),
    /// Le résultat d'une fonction qui ne rend rien.
    Void,
}

struct Parser<'a, 'm> {
    info: &'a DebugInfo,
    tokens: Vec<Token>,
    at: usize,
    pc: u32,
    ctx: &'a mut Context<'m>,
    steps: u32,
}

/// Évalue `text` dans le contexte d'un cadre (`pc` pour la visibilité des variables, `ctx` pour les registres et la mémoire).
pub fn evaluate(
    info: &DebugInfo,
    text: &str,
    pc: u32,
    ctx: &mut Context<'_>,
    frame_base: Option<&[u8]>,
) -> Result<Value, EvalError> {
    let tokens = lex(text)?;
    if tokens.is_empty() {
        return fail("expression vide");
    }
    // La base du cadre est évaluée une fois (elle peut être une expression qui lit un registre).
    if ctx.frame_base.is_none()
        && let Some(expr) = frame_base
    {
        match locate(expr, ctx) {
            Ok(Place::Register(r)) => ctx.frame_base = ctx.regs.get(usize::from(r)).copied(),
            Ok(Place::Address(a)) => ctx.frame_base = Some(a),
            _ => {}
        }
    }
    let mut p = Parser {
        info,
        tokens,
        at: 0,
        pc,
        ctx,
        steps: 0,
    };
    let result = p.expression()?;
    if p.at < p.tokens.len() {
        return fail("texte inattendu à la fin de l'expression");
    }
    p.finish(result)
}

fn binary_precedence(op: &str) -> Option<u8> {
    Some(match op {
        "||" => 1,
        "&&" => 2,
        "|" => 3,
        "^" => 4,
        "&" => 5,
        "==" | "!=" => 6,
        "<" | ">" | "<=" | ">=" => 7,
        "<<" | ">>" => 8,
        "+" | "-" => 9,
        "*" | "/" | "%" => 10,
        _ => return None,
    })
}

const ASSIGN: &[&str] = &[
    "=", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<=", ">>=",
];
const TYPE_WORDS: &[&str] = &[
    "const", "volatile", "unsigned", "signed", "long", "short", "int", "char", "float", "double",
    "struct", "union", "enum", "void", "_Bool", "bool",
];

impl Parser<'_, '_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn peek_punct(&self) -> Option<&'static str> {
        match self.peek() {
            Some(Token::Punct(p)) => Some(p),
            _ => None,
        }
    }

    fn eat(&mut self, punct: &str) -> bool {
        if self.peek_punct() == Some(punct) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, punct: &str) -> Result<(), EvalError> {
        if self.eat(punct) {
            Ok(())
        } else {
            fail(format!("« {punct} » attendu"))
        }
    }

    // ---- grammaire ---------------------------------------------------------------------------------------------

    fn expression(&mut self) -> Result<Operand, EvalError> {
        let mut value = self.assignment()?;
        while self.eat(",") {
            value = self.assignment()?;
        }
        Ok(value)
    }

    fn assignment(&mut self) -> Result<Operand, EvalError> {
        let left = self.ternary()?;
        let Some(op) = self.peek_punct().filter(|p| ASSIGN.contains(p)) else {
            return Ok(left);
        };
        self.at += 1;
        let right = self.assignment()?;
        let value = if op == "=" {
            right
        } else {
            let current = left.clone();
            self.binary(&op[..op.len() - 1], current, right)?
        };
        self.store(&left, &value)?;
        Ok(left)
    }

    fn ternary(&mut self) -> Result<Operand, EvalError> {
        let condition = self.binary_level(1)?;
        if !self.eat("?") {
            return Ok(condition);
        }
        let yes = self.assignment()?;
        self.expect(":")?;
        let no = self.ternary()?;
        Ok(if self.int_of(&condition)? != 0 {
            yes
        } else {
            no
        })
    }

    fn binary_level(&mut self, min: u8) -> Result<Operand, EvalError> {
        let mut left = self.unary()?;
        while let Some(op) = self.peek_punct() {
            let Some(prec) = binary_precedence(op) else {
                break;
            };
            if prec < min {
                break;
            }
            self.at += 1;
            let right = self.binary_level(prec + 1)?;
            left = self.binary(op, left, right)?;
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Operand, EvalError> {
        self.steps += 1;
        if self.steps > 1024 {
            return fail("expression trop longue");
        }
        if self.eat("-") {
            return match self.unary()? {
                Operand::Float(f) => Ok(Operand::Float(-f)),
                other => match self.float_of(&other) {
                    Some(f) => Ok(Operand::Float(-f)),
                    None => Ok(Operand::Int(self.int_of(&other)?.wrapping_neg())),
                },
            };
        }
        if self.eat("+") {
            return self.unary();
        }
        if self.eat("!") {
            let v = self.unary()?;
            return Ok(Operand::Int(i64::from(self.int_of(&v)? == 0)));
        }
        if self.eat("~") {
            let v = self.unary()?;
            return Ok(Operand::Int(!self.int_of(&v)?));
        }
        if self.eat("*") {
            let v = self.unary()?;
            return self.deref(v);
        }
        if self.eat("&") {
            return match self.unary()? {
                Operand::Object {
                    ty,
                    place: Place::Address(address),
                } => Ok(Operand::Address {
                    target: ty,
                    address,
                }),
                Operand::Func { address, .. } => Ok(Operand::Address {
                    target: None,
                    address,
                }),
                _ => fail("on ne peut prendre l'adresse que d'un objet en mémoire"),
            };
        }
        for (symbol, delta) in [("++", 1), ("--", -1)] {
            if self.eat(symbol) {
                let target = self.unary()?;
                let updated = self.step_value(&target, delta)?;
                self.store(&target, &updated)?;
                return Ok(target);
            }
        }
        if matches!(self.peek(), Some(Token::Ident(w)) if w == "sizeof") {
            self.at += 1;
            return self.sizeof();
        }
        // `(type) expression` : une conversion.
        if self.peek_punct() == Some("(")
            && let Some((ty, after)) = self.try_type(self.at + 1)?
            && self.tokens.get(after) == Some(&Token::Punct(")"))
        {
            self.at = after + 1;
            let operand = self.unary()?;
            return self.cast(ty, operand);
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Result<Operand, EvalError> {
        let mut v = self.primary()?;
        loop {
            if self.eat(".") || self.peek_punct() == Some("->") {
                let arrow = self.peek_punct() == Some("->");
                if arrow {
                    self.at += 1;
                }
                let Some(Token::Ident(name)) = self.peek().cloned() else {
                    return fail("nom de champ attendu après « . » ou « -> »");
                };
                self.at += 1;
                let object = if arrow { self.deref(v)? } else { v };
                // `objet.méthode(args)` : l'appel d'une fonction membre.
                if self.peek_punct() == Some("(") {
                    self.at += 1;
                    let args = self.arguments()?;
                    v = self.call_method(object, &name, &args)?;
                } else {
                    v = self.field(object, &name)?;
                }
            } else if self.eat("[") {
                let index = self.expression()?;
                self.expect("]")?;
                v = self.index(v, &index)?;
            } else if self.eat("(") {
                let args = self.arguments()?;
                v = self.call(v, &args)?;
            } else if let Some(delta) = [("++", 1), ("--", -1)]
                .into_iter()
                .find_map(|(s, d)| (self.peek_punct() == Some(s)).then_some(d))
            {
                self.at += 1;
                let old = self.snapshot(&v)?;
                let updated = self.step_value(&v, delta)?;
                self.store(&v, &updated)?;
                v = old;
            } else {
                return Ok(v);
            }
        }
    }

    /// Les arguments d'un appel, après la parenthèse ouvrante.
    fn arguments(&mut self) -> Result<Vec<Operand>, EvalError> {
        let mut args = Vec::new();
        if !self.eat(")") {
            loop {
                args.push(self.assignment()?);
                if self.eat(")") {
                    break;
                }
                self.expect(",")?;
            }
        }
        Ok(args)
    }

    fn primary(&mut self) -> Result<Operand, EvalError> {
        match self.peek().cloned() {
            Some(Token::Number(n)) => {
                self.at += 1;
                Ok(Operand::Int(n))
            }
            Some(Token::Float(f)) => {
                self.at += 1;
                Ok(Operand::Float(f))
            }
            Some(Token::Punct("(")) => {
                self.at += 1;
                let v = self.expression()?;
                self.expect(")")?;
                Ok(v)
            }
            Some(Token::Ident(mut name)) => {
                self.at += 1;
                // Un nom qualifié : `shapes::Shape::counter`, `main::GLOBAL_TOTAL`.
                while self.peek_punct() == Some("::")
                    && let Some(Token::Ident(next)) = self.tokens.get(self.at + 1)
                {
                    name = format!("{name}::{next}");
                    self.at += 2;
                }
                self.variable(&name)
            }
            Some(_) => fail("expression inattendue"),
            None => fail("expression incomplète"),
        }
    }

    // ---- types : `(type)` et `sizeof(type)` --------------------------------------------------------------------

    /// Un nom qui commence à `from` : des identifiants séparés par `::`, avec des arguments de modèle entre `<` et `>`
    /// (`shapes::Rect`, `Stack<int, 4>`). Rend le nom, écrit sans espaces, et la position qui suit.
    fn qualified_name_at(&self, from: usize) -> Option<(String, usize)> {
        let mut i = from;
        let mut name = String::new();
        loop {
            let Some(Token::Ident(part)) = self.tokens.get(i) else {
                return None;
            };
            name += part;
            i += 1;
            if self.tokens.get(i) == Some(&Token::Punct("<")) {
                // Les arguments du modèle, jusqu'au `>` qui ferme (les `>>` ferment deux niveaux).
                let mut depth = 0i32;
                let mut j = i;
                let mut text = String::new();
                while let Some(token) = self.tokens.get(j) {
                    match token {
                        Token::Punct("<") => depth += 1,
                        Token::Punct(">") => depth -= 1,
                        Token::Punct(">>") => depth -= 2,
                        _ => {}
                    }
                    text += &match token {
                        Token::Number(n) => n.to_string(),
                        Token::Float(f) => f.to_string(),
                        Token::Ident(w) => format!("{w} "),
                        Token::Punct(p) => (*p).to_owned(),
                    };
                    j += 1;
                    if depth <= 0 {
                        break;
                    }
                }
                if depth != 0 {
                    return None;
                }
                name += text.trim_end();
                i = j;
            }
            if self.tokens.get(i) == Some(&Token::Punct("::"))
                && matches!(self.tokens.get(i + 1), Some(Token::Ident(_)))
            {
                name += "::";
                i += 1;
            } else {
                return Some((name, i));
            }
        }
    }

    /// Un nom de type qui commence à `from` : le type (`None` : `void`) et la position qui suit.
    fn try_type(&self, from: usize) -> Result<Option<(Option<TypeId>, usize)>, EvalError> {
        let mut words: Vec<String> = Vec::new();
        let mut i = from;
        while let Some(Token::Ident(w)) = self.tokens.get(i) {
            let is_word = TYPE_WORDS.contains(&w.as_str());
            if is_word {
                words.push(w.clone());
                i += 1;
                if matches!(w.as_str(), "struct" | "union" | "enum") {
                    let Some((tag, after)) = self.qualified_name_at(i) else {
                        return fail("nom attendu après struct, union ou enum");
                    };
                    words.push(tag);
                    i = after;
                    break;
                }
                continue;
            }
            // Un nom de type défini par le programme (`Point`, `uint8_t`, `shapes::Rect`, `Stack<int, 4>`) : seulement s'il n'est pas
            // aussi une variable ou une fonction visible.
            if !words
                .iter()
                .all(|x| matches!(x.as_str(), "const" | "volatile"))
            {
                break;
            }
            let Some((name, after)) = self.qualified_name_at(i) else {
                break;
            };
            let shadowed = self.info.find_variable(self.pc, &name).is_some()
                || !self.info.functions_named(&name).is_empty();
            if shadowed || self.info.type_named(&name, None).is_none() {
                break;
            }
            words.push(name);
            i = after;
            break;
        }
        if words
            .iter()
            .all(|w| matches!(w.as_str(), "const" | "volatile"))
        {
            return Ok(None);
        }
        // Les mots de qualité ne changent pas le type.
        let core: Vec<&str> = words
            .iter()
            .map(String::as_str)
            .filter(|w| !matches!(*w, "const" | "volatile"))
            .collect();
        let base: Option<TypeId> = match core.as_slice() {
            ["void"] => None,
            [kind @ ("struct" | "union" | "enum"), tag] => Some(
                self.info
                    .type_named(tag, Some(kind))
                    .ok_or_else(|| EvalError(format!("type inconnu : {kind} {tag}")))?,
            ),
            [name] if !TYPE_WORDS.contains(name) => Some(
                self.info
                    .type_named(name, None)
                    .ok_or_else(|| EvalError(format!("type inconnu : {name}")))?,
            ),
            words => Some(
                self.info
                    .base_named(words)
                    .ok_or_else(|| EvalError(format!("type inconnu : {}", words.join(" "))))?,
            ),
        };
        let mut ty = base;
        while matches!(self.tokens.get(i), Some(Token::Punct("*"))) {
            i += 1;
            while matches!(self.tokens.get(i), Some(Token::Ident(w)) if w == "const" || w == "volatile")
            {
                i += 1;
            }
            ty = Some(
                self.info
                    .pointer_to(ty)
                    .ok_or_else(|| EvalError("ce type de pointeur n'est pas connu".into()))?,
            );
        }
        Ok(Some((ty, i)))
    }

    fn sizeof(&mut self) -> Result<Operand, EvalError> {
        if self.peek_punct() == Some("(")
            && let Some((ty, after)) = self.try_type(self.at + 1)?
            && self.tokens.get(after) == Some(&Token::Punct(")"))
        {
            self.at = after + 1;
            return Ok(Operand::Int(i64::from(
                ty.map_or(1, |t| self.info.size_of(t)),
            )));
        }
        let v = self.unary()?;
        Ok(Operand::Int(i64::from(match &v {
            Operand::Object { ty: Some(t), .. } | Operand::Typed { ty: t, .. } => {
                self.info.size_of(*t)
            }
            Operand::Float(_) => 8,
            Operand::Address { .. } | Operand::Func { .. } => 4,
            _ => 4,
        })))
    }

    // ---- accès aux valeurs -----------------------------------------------------------------------------------------

    fn variable(&mut self, name: &str) -> Result<Operand, EvalError> {
        if let Some(reg) = name.strip_prefix('$') {
            let index = match reg {
                "sp" => 13,
                "lr" => 14,
                "pc" => 15,
                r => r
                    .strip_prefix('r')
                    .and_then(|n| n.parse::<usize>().ok())
                    .filter(|n| *n < 16)
                    .ok_or_else(|| EvalError(format!("registre inconnu : {name}")))?,
            };
            return Ok(Operand::Register(index));
        }
        if let Some(var) = self.info.find_variable(self.pc, name) {
            if let Some(c) = var.const_value
                && var.location == super::Location::None
            {
                return Ok(Operand::Int(c));
            }
            let Some(expr) = var.location.at(self.pc) else {
                return Ok(Operand::Object {
                    ty: var.ty,
                    place: Place::OptimizedOut,
                });
            };
            let place = locate(expr, self.ctx).map_err(EvalError)?;
            return Ok(self.unref(Operand::Object { ty: var.ty, place }));
        }
        let functions = self.info.functions_named(name);
        match functions.as_slice() {
            [] => {}
            [f] => {
                return Ok(Operand::Func {
                    address: f.low,
                    ret: f.ret,
                });
            }
            many => {
                return Ok(Operand::Overloads(many.iter().map(|f| f.low).collect()));
            }
        }
        // Dans une fonction membre, un nom de champ (`size_`) vaut `this->size_`.
        if self.info.find_variable(self.pc, "this").is_some() && name != "this" {
            let this = self.variable("this")?;
            if let Ok(object) = self.deref(this)
                && let Ok(found) = self.field(object, name)
            {
                return Ok(found);
            }
        }
        // Une constante d'énumération.
        for i in 0..self.info.type_count() {
            if let TypeKind::Enum { variants, .. } = &self.info.ty(i).kind
                && let Some((_, v)) = variants.iter().find(|(n, _)| n == name)
            {
                return Ok(Operand::Int(*v));
            }
        }
        fail(format!("symbole inconnu : {name}"))
    }

    fn read(&mut self, ty: Option<TypeId>, place: Place) -> Value {
        read_value(self.info, ty, place, self.ctx)
    }

    fn int_of(&mut self, v: &Operand) -> Result<i64, EvalError> {
        match v {
            Operand::Int(n) => Ok(*n),
            Operand::Float(f) => Ok(*f as i64),
            Operand::Typed { num: Num::I(n), .. } => Ok(*n),
            Operand::Typed { num: Num::F(f), .. } => Ok(*f as i64),
            Operand::Address { address, .. } | Operand::Func { address, .. } => {
                Ok(i64::from(*address))
            }
            Operand::Register(i) => Ok(i64::from(self.ctx.regs[*i])),
            Operand::Void => fail("la fonction ne rend rien"),
            Operand::Overloads(list) => Ok(i64::from(list[0])),
            Operand::Object { ty, place } => {
                let value = self.read(*ty, place.clone());
                match value.kind {
                    ValueKind::Int(n) | ValueKind::Char(n) => Ok(n),
                    ValueKind::UInt(n) => Ok(n as i64),
                    ValueKind::Bool(b) => Ok(i64::from(b)),
                    ValueKind::Enum { value, .. } => Ok(value),
                    ValueKind::Pointer { address, .. } => Ok(i64::from(address)),
                    ValueKind::Float(f) => Ok(f as i64),
                    ValueKind::Function(a) => Ok(i64::from(a)),
                    // Un tableau dans une expression vaut l'adresse de son premier élément.
                    ValueKind::Array { .. } => match place {
                        Place::Address(a) => Ok(i64::from(*a)),
                        _ => fail("tableau hors mémoire"),
                    },
                    ValueKind::Unavailable(why) => fail(why),
                    ValueKind::OptimizedOut => fail("variable optimisée"),
                    _ => fail("un entier est attendu"),
                }
            }
        }
    }

    fn float_of(&mut self, v: &Operand) -> Option<f64> {
        match v {
            Operand::Float(f) => Some(*f),
            Operand::Typed { num: Num::F(f), .. } => Some(*f),
            Operand::Object { ty, place } => match self.read(*ty, place.clone()).kind {
                ValueKind::Float(f) => Some(f),
                _ => None,
            },
            _ => None,
        }
    }

    /// Le contenu d'un objet, détaché de son emplacement (pour `x++` : la valeur d'avant).
    fn snapshot(&mut self, v: &Operand) -> Result<Operand, EvalError> {
        if let Some(f) = self.float_of(v) {
            return Ok(Operand::Float(f));
        }
        if let Operand::Address { .. } = v {
            return Ok(v.clone());
        }
        Ok(Operand::Int(self.int_of(v)?))
    }

    fn deref(&mut self, v: Operand) -> Result<Operand, EvalError> {
        let (target, address) = match &v {
            Operand::Address { target, address } => (*target, *address),
            Operand::Func { .. } => return Ok(v),
            Operand::Object { ty, place } => {
                let value = self.read(*ty, place.clone());
                match value.kind {
                    ValueKind::Pointer {
                        address, target, ..
                    } => (target, address),
                    ValueKind::Array { .. } => match (place, ty) {
                        (Place::Address(a), Some(t)) => {
                            match &self.info.ty(self.info.strip(*t)).kind {
                                TypeKind::Array { elem, .. } => (Some(*elem), *a),
                                _ => return fail("seul un pointeur peut être déréférencé"),
                            }
                        }
                        _ => return fail("tableau hors mémoire"),
                    },
                    _ => return fail("seul un pointeur peut être déréférencé"),
                }
            }
            Operand::Register(_) | Operand::Int(_) | Operand::Typed { .. } => {
                // Un entier traité comme une adresse : `*(int *)0x20000000` passe par une conversion, sinon c'est une erreur.
                return fail(
                    "seul un pointeur peut être déréférencé (utilisez une conversion : *(int *)adresse)",
                );
            }
            _ => return fail("seul un pointeur peut être déréférencé"),
        };
        if address == 0 {
            return fail("pointeur nul");
        }
        Ok(Operand::Object {
            ty: target,
            place: Place::Address(address),
        })
    }

    /// Une référence (`int &`, `&T`) se traverse toujours : on travaille sur l'objet qu'elle désigne.
    fn unref(&mut self, v: Operand) -> Operand {
        if let Operand::Object {
            ty: Some(t),
            place: Place::Address(a),
        } = &v
            && let TypeKind::Reference(target) = self.info.ty(self.info.strip(*t)).kind.clone()
            && let Some(bytes) = self.ctx.memory.read(*a, 4)
            && let Ok(bytes) = <[u8; 4]>::try_from(bytes)
        {
            let pointer = u32::from_le_bytes(bytes);
            if pointer != 0 {
                return Operand::Object {
                    ty: target,
                    place: Place::Address(pointer),
                };
            }
        }
        v
    }

    /// Le champ `name` d'une structure, cherché aussi dans ses classes de base. Un pointeur ou une référence vers la structure
    /// se traverse (`p.x` vaut `p->x`).
    fn field(&mut self, v: Operand, name: &str) -> Result<Operand, EvalError> {
        let mut v = self.unref(v);
        // Un pointeur vers une structure : on l'ouvre.
        if let Operand::Object { ty: Some(t), .. } = &v
            && matches!(self.info.ty(self.info.strip(*t)).kind, TypeKind::Pointer(_))
        {
            v = self.deref(v)?;
        }
        let Operand::Object {
            ty: Some(ty),
            place,
        } = v
        else {
            return fail("ce n'est pas une structure");
        };
        let Place::Address(base) = place else {
            return fail("structure hors mémoire");
        };
        let Some((field_ty, offset)) = self.find_field(ty, name, 0) else {
            let stripped = self.info.strip(ty);
            return match &self.info.ty(stripped).kind {
                TypeKind::Struct { .. } => fail(format!("pas de champ « {name} »")),
                _ => fail("ce n'est pas une structure"),
            };
        };
        Ok(self.unref(Operand::Object {
            ty: Some(field_ty),
            place: Place::Address(base.wrapping_add(offset)),
        }))
    }

    /// Le type et la position (depuis le début de `ty`) du champ `name`, dans `ty` ou ses bases.
    fn find_field(&self, ty: TypeId, name: &str, depth: u32) -> Option<(TypeId, u32)> {
        if depth > 8 {
            return None;
        }
        let TypeKind::Struct { fields, .. } = &self.info.ty(self.info.strip(ty)).kind else {
            return None;
        };
        if let Some(f) = fields.iter().find(|f| f.name == name && !f.base) {
            return Some((f.ty, f.offset));
        }
        fields.iter().filter(|f| f.base).find_map(|f| {
            self.find_field(f.ty, name, depth + 1)
                .map(|(t, o)| (t, o + f.offset))
        })
    }

    fn index(&mut self, v: Operand, index: &Operand) -> Result<Operand, EvalError> {
        let i = self.int_of(index)?;
        let (elem, base) = match &v {
            Operand::Object {
                ty: Some(ty),
                place,
            } => {
                let stripped = self.info.strip(*ty);
                match &self.info.ty(stripped).kind {
                    TypeKind::Array { elem, .. } => {
                        let Place::Address(a) = place else {
                            return fail("tableau hors mémoire");
                        };
                        (Some(*elem), *a)
                    }
                    TypeKind::Pointer(target) => {
                        let value = self.read(Some(*ty), place.clone());
                        let ValueKind::Pointer { address, .. } = value.kind else {
                            return fail("pointeur illisible");
                        };
                        (*target, address)
                    }
                    _ => return fail("ce n'est ni un tableau ni un pointeur"),
                }
            }
            Operand::Address { target, address } => (*target, *address),
            _ => return fail("ce n'est ni un tableau ni un pointeur"),
        };
        let size = elem.map_or(1, |e| self.info.size_of(e).max(1));
        Ok(Operand::Object {
            ty: elem,
            place: Place::Address(base.wrapping_add((i as i32 as u32).wrapping_mul(size))),
        })
    }

    // ---- opérations ----------------------------------------------------------------------------------------------

    fn binary(&mut self, op: &str, left: Operand, right: Operand) -> Result<Operand, EvalError> {
        // Arithmétique de pointeurs : `p + n` avance de `n` éléments ; `p - q` donne un nombre d'éléments.
        let pointer = |this: &mut Self, v: &Operand| -> Option<(Option<TypeId>, u32)> {
            match v {
                Operand::Address { target, address } => Some((*target, *address)),
                Operand::Object { ty: Some(t), place } => {
                    match &this.info.ty(this.info.strip(*t)).kind {
                        TypeKind::Pointer(target) => {
                            match this.read(Some(*t), place.clone()).kind {
                                ValueKind::Pointer { address, .. } => Some((*target, address)),
                                _ => None,
                            }
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        };
        if matches!(op, "+" | "-")
            && let Some((target, address)) = pointer(self, &left)
        {
            let size = target.map_or(1, |t| self.info.size_of(t).max(1));
            if let Some((_, other)) = pointer(self, &right) {
                if op == "-" {
                    return Ok(Operand::Int(
                        i64::from(address.wrapping_sub(other) as i32) / i64::from(size),
                    ));
                }
                return fail("on n'additionne pas deux pointeurs");
            }
            let n = self.int_of(&right)?;
            let delta = (n as i32).wrapping_mul(size as i32) as u32;
            let address = if op == "+" {
                address.wrapping_add(delta)
            } else {
                address.wrapping_sub(delta)
            };
            return Ok(Operand::Address { target, address });
        }
        let (lf, rf) = (self.float_of(&left), self.float_of(&right));
        if lf.is_some() || rf.is_some() {
            let a = match lf {
                Some(f) => f,
                None => self.int_of(&left)? as f64,
            };
            let b = match rf {
                Some(f) => f,
                None => self.int_of(&right)? as f64,
            };
            return match op {
                "+" => Ok(Operand::Float(a + b)),
                "-" => Ok(Operand::Float(a - b)),
                "*" => Ok(Operand::Float(a * b)),
                "/" => Ok(Operand::Float(a / b)),
                "<" => Ok(Operand::Int(i64::from(a < b))),
                ">" => Ok(Operand::Int(i64::from(a > b))),
                "<=" => Ok(Operand::Int(i64::from(a <= b))),
                ">=" => Ok(Operand::Int(i64::from(a >= b))),
                "==" => Ok(Operand::Int(i64::from(a == b))),
                "!=" => Ok(Operand::Int(i64::from(a != b))),
                "&&" => Ok(Operand::Int(i64::from(a != 0.0 && b != 0.0))),
                "||" => Ok(Operand::Int(i64::from(a != 0.0 || b != 0.0))),
                _ => fail(format!(
                    "l'opérateur {op} ne s'applique pas à des nombres à virgule"
                )),
            };
        }
        let (a, b) = (self.int_of(&left)?, self.int_of(&right)?);
        let r = match op {
            "+" => a.wrapping_add(b),
            "-" => a.wrapping_sub(b),
            "*" => a.wrapping_mul(b),
            "/" => a
                .checked_div(b)
                .ok_or_else(|| EvalError("division par zéro".into()))?,
            "%" => a
                .checked_rem(b)
                .ok_or_else(|| EvalError("division par zéro".into()))?,
            "<<" => a.wrapping_shl(b as u32),
            ">>" => a.wrapping_shr(b as u32),
            "&" => a & b,
            "|" => a | b,
            "^" => a ^ b,
            "<" => i64::from(a < b),
            ">" => i64::from(a > b),
            "<=" => i64::from(a <= b),
            ">=" => i64::from(a >= b),
            "==" => i64::from(a == b),
            "!=" => i64::from(a != b),
            "&&" => i64::from(a != 0 && b != 0),
            "||" => i64::from(a != 0 || b != 0),
            _ => return fail(format!("opérateur non géré : {op}")),
        };
        Ok(Operand::Int(r))
    }

    /// La valeur de `v` plus `delta` (un élément pour un pointeur) : pour `++` et `--`.
    fn step_value(&mut self, v: &Operand, delta: i64) -> Result<Operand, EvalError> {
        self.binary(
            if delta > 0 { "+" } else { "-" },
            v.clone(),
            Operand::Int(1),
        )
    }

    /// Convertit `operand` vers `ty` (`None` : `void`).
    fn cast(&mut self, ty: Option<TypeId>, operand: Operand) -> Result<Operand, EvalError> {
        let Some(ty) = ty else {
            return Ok(Operand::Void);
        };
        let stripped = self.info.strip(ty);
        let size = self.info.size_of(stripped);
        match self.info.ty(stripped).kind.clone() {
            TypeKind::Pointer(target) => Ok(Operand::Address {
                target,
                address: self.int_of(&operand)? as u32,
            }),
            TypeKind::Base(Encoding::Float) => {
                let f = match self.float_of(&operand) {
                    Some(f) => f,
                    None => self.int_of(&operand)? as f64,
                };
                Ok(Operand::Typed {
                    ty,
                    num: Num::F(if size == 4 { f64::from(f as f32) } else { f }),
                })
            }
            TypeKind::Base(Encoding::Bool) => {
                let nonzero = match self.float_of(&operand) {
                    Some(f) => f != 0.0,
                    None => self.int_of(&operand)? != 0,
                };
                Ok(Operand::Typed {
                    ty,
                    num: Num::I(i64::from(nonzero)),
                })
            }
            TypeKind::Base(encoding) => {
                let raw = match self.float_of(&operand) {
                    Some(f) => f as i64,
                    None => self.int_of(&operand)?,
                };
                let signed = matches!(encoding, Encoding::Signed | Encoding::SignedChar);
                Ok(Operand::Typed {
                    ty,
                    num: Num::I(truncate(raw, size, signed)),
                })
            }
            TypeKind::Enum { .. } => Ok(Operand::Typed {
                ty,
                num: Num::I(truncate(self.int_of(&operand)?, size.max(1), true)),
            }),
            _ => fail(
                "cette conversion n'est pas gérée (seuls les nombres, les pointeurs et les énumérations)",
            ),
        }
    }

    /// Appelle une fonction de l'application sur l'appareil.
    /// Appelle la fonction membre `name` de l'objet `object` : par la table virtuelle si elle est virtuelle, sinon par son symbole.
    fn call_method(
        &mut self,
        object: Operand,
        name: &str,
        args: &[Operand],
    ) -> Result<Operand, EvalError> {
        let mut object = self.unref(object);
        // Un pointeur vers l'objet (`self.area()` avec `&self`, `p.méthode()`) s'ouvre comme avec `->`.
        if let Operand::Object { ty: Some(t), .. } = &object
            && matches!(self.info.ty(self.info.strip(*t)).kind, TypeKind::Pointer(_))
        {
            object = self.deref(object)?;
        }
        let Operand::Object {
            ty: Some(ty),
            place: Place::Address(address),
        } = object
        else {
            return fail("ce n'est pas un objet en mémoire");
        };
        let type_name = self.info.type_name(Some(ty));
        // D'abord les fonctions membres que décrit le type (C++), puis une fonction nommée `Type::méthode` (Rust, hors classe).
        let target: Option<(u32, Option<TypeId>, u32)> = if let Some((method, this_offset)) =
            self.info.find_method(ty, name, args.len())
        {
            let this = address.wrapping_add(this_offset);
            let function = if let Some(slot) = method.virtual_slot {
                let vptr = self
                    .read_word(this)
                    .ok_or_else(|| EvalError("table virtuelle illisible".into()))?;
                self.read_word(vptr.wrapping_add(4 * slot))
                    .map(|a| a & !1)
                    .ok_or_else(|| EvalError("table virtuelle illisible".into()))?
            } else {
                self.info
                    .method_address(method)
                    .ok_or_else(|| EvalError(format!("la méthode {name} n'existe pas dans le programme (elle n'a pas été compilée)")))?
            };
            Some((
                function,
                method.ret,
                if method.is_static { 0 } else { this },
            ))
        } else {
            self.info
                .method_function(&type_name, name, args.len())
                .map(|f| (f.low, f.ret, address))
        };
        let Some((function, ret, this)) = target else {
            return fail(format!("pas de méthode « {name} » pour {type_name}"));
        };
        let mut all = Vec::with_capacity(args.len() + 1);
        if this != 0 {
            all.push(Operand::Int(i64::from(this)));
        }
        all.extend(args.iter().cloned());
        self.call(
            Operand::Func {
                address: function,
                ret,
            },
            &all,
        )
    }

    fn read_word(&mut self, address: u32) -> Option<u32> {
        let bytes = self.ctx.memory.read(address, 4)?;
        Some(u32::from_le_bytes(bytes.try_into().ok()?))
    }

    fn call(&mut self, callee: Operand, args: &[Operand]) -> Result<Operand, EvalError> {
        let (address, ret) = match &callee {
            Operand::Func { address, ret } => (*address, *ret),
            // Plusieurs fonctions de même nom : celle qui convient aux arguments (nombre, entiers ou flottants).
            Operand::Overloads(list) => {
                let candidates: Vec<&super::Function> = list
                    .iter()
                    .filter_map(|a| self.info.function_at_start(*a))
                    .collect();
                let kinds: Vec<bool> = args.iter().map(|a| self.float_of(a).is_some()).collect();
                let chosen = self
                    .info
                    .best_overload(&candidates, &kinds)
                    .ok_or_else(|| EvalError("aucune surcharge ne convient".into()))?;
                (chosen.low, chosen.ret)
            }
            Operand::Address { address, target } => (*address, self.function_return(*target)),
            Operand::Object { ty: Some(t), place } => {
                let ret = match &self.info.ty(self.info.strip(*t)).kind {
                    TypeKind::Pointer(target) => self.function_return(*target),
                    TypeKind::Function { ret, .. } => *ret,
                    _ => return fail("ce n'est pas une fonction"),
                };
                match self.read(Some(*t), place.clone()).kind {
                    ValueKind::Pointer { address, .. } => (address, ret),
                    ValueKind::Function(address) => (address, ret),
                    _ => return fail("ce n'est pas une fonction"),
                }
            }
            _ => return fail("ce n'est pas une fonction"),
        };
        // Chaque argument est converti vers le type du paramètre quand on le connaît (entier, flottant, double).
        let params: Vec<Option<TypeId>> = self
            .info
            .function_at_start(address)
            .map(|f| f.params.iter().map(|p| p.ty).collect())
            .unwrap_or_default();
        // Une méthode reçoit `this` en premier : il est déjà dans `args`, comme le paramètre artificiel du même rang.
        let mut converted = Vec::with_capacity(args.len());
        for (i, arg) in args.iter().enumerate() {
            let want = params
                .get(i)
                .copied()
                .flatten()
                .map(|t| (self.info.strip(t), self.info.size_of(t)));
            let float_kind = want.and_then(|(t, size)| match &self.info.ty(t).kind {
                TypeKind::Base(Encoding::Float) => Some(size),
                _ => None,
            });
            let value = self.float_of(arg);
            converted.push(match (float_kind, value) {
                (Some(4), Some(f)) => CallArg::Float(f as f32),
                (Some(4), None) => CallArg::Float(self.int_of(arg)? as f32),
                (Some(_), Some(f)) => CallArg::Double(f),
                (Some(_), None) => CallArg::Double(self.int_of(arg)? as f64),
                (None, Some(f)) if want.is_none() => CallArg::Double(f),
                (None, Some(f)) => CallArg::Int(f as i64 as u32),
                (None, None) => CallArg::Int(self.int_of(arg)? as u32),
            });
        }
        let result = self
            .ctx
            .memory
            .call(address, &converted)
            .map_err(EvalError)?;
        match ret {
            None => Ok(Operand::Void),
            Some(t)
                if matches!(
                    self.info.ty(self.info.strip(t)).kind,
                    TypeKind::Struct { .. } | TypeKind::Array { .. }
                ) =>
            {
                fail(
                    "cette fonction rend une structure ou un tableau : ce n'est pas géré (elle a été exécutée)",
                )
            }
            Some(t) => {
                let stripped = self.info.strip(t);
                let size = self.info.size_of(t);
                match self.info.ty(stripped).kind {
                    TypeKind::Base(Encoding::Float) if size == 4 => self.cast(
                        Some(t),
                        Operand::Float(f64::from(f32::from_bits(result.s0))),
                    ),
                    TypeKind::Base(Encoding::Float) => self.cast(
                        Some(t),
                        Operand::Float(f64::from_bits(
                            u64::from(result.s0) | (u64::from(result.s1) << 32),
                        )),
                    ),
                    TypeKind::Base(_) if size == 8 => self.cast(
                        Some(t),
                        Operand::Int((u64::from(result.r0) | (u64::from(result.r1) << 32)) as i64),
                    ),
                    _ => self.cast(Some(t), Operand::Int(i64::from(result.r0))),
                }
            }
        }
    }

    fn function_return(&self, target: Option<TypeId>) -> Option<TypeId> {
        match target.map(|t| &self.info.ty(self.info.strip(t)).kind) {
            Some(TypeKind::Function { ret, .. }) => *ret,
            _ => None,
        }
    }

    /// Écrit `value` dans l'objet `target`, converti à son type.
    fn store(&mut self, target: &Operand, value: &Operand) -> Result<(), EvalError> {
        match target {
            Operand::Register(index) => {
                let n = self.int_of(value)? as u32;
                if !self.ctx.memory.set_register(*index, n) {
                    return fail("ce registre ne peut pas être modifié");
                }
                self.ctx.regs[*index] = n;
                Ok(())
            }
            Operand::Object {
                ty: Some(ty),
                place: Place::Address(address),
            } => {
                let stripped = self.info.strip(*ty);
                let size = self.info.size_of(stripped).clamp(1, 8) as usize;
                let bytes: Vec<u8> = match &self.info.ty(stripped).kind {
                    TypeKind::Base(Encoding::Float) => {
                        let f = match self.float_of(value) {
                            Some(f) => f,
                            None => self.int_of(value)? as f64,
                        };
                        if size == 4 {
                            (f as f32).to_le_bytes().to_vec()
                        } else {
                            f.to_le_bytes().to_vec()
                        }
                    }
                    TypeKind::Base(_) | TypeKind::Enum { .. } | TypeKind::Pointer(_) => {
                        let n = match self.float_of(value) {
                            Some(f) => f as i64,
                            None => self.int_of(value)?,
                        };
                        n.to_le_bytes()[..size].to_vec()
                    }
                    _ => {
                        return fail(
                            "on ne peut écrire qu'un nombre, un pointeur ou une énumération",
                        );
                    }
                };
                if self.ctx.memory.write(*address, &bytes) {
                    Ok(())
                } else {
                    fail(format!("mémoire non inscriptible à 0x{address:08x}"))
                }
            }
            Operand::Object {
                place: Place::Register(_),
                ..
            } => fail("cette variable est dans un registre : on ne peut pas la modifier"),
            Operand::Object { .. } => {
                fail("cette valeur n'est pas en mémoire : on ne peut pas la modifier")
            }
            _ => fail("on ne peut pas modifier cette valeur"),
        }
    }

    /// Le résultat final, sous forme de valeur lisible.
    fn finish(&mut self, v: Operand) -> Result<Value, EvalError> {
        Ok(match v {
            Operand::Int(n) => Value {
                ty: None,
                type_name: "int".into(),
                kind: ValueKind::Int(n),
                place: None,
                size: 4,
            },
            Operand::Float(f) => Value {
                ty: None,
                type_name: "double".into(),
                kind: ValueKind::Float(f),
                place: None,
                size: 8,
            },
            Operand::Overloads(list) => Value {
                ty: None,
                type_name: format!("{} surcharges", list.len()),
                kind: ValueKind::Function(list[0]),
                place: Some(Place::Address(list[0])),
                size: 0,
            },
            Operand::Void => Value {
                ty: None,
                type_name: "void".into(),
                kind: ValueKind::Unavailable("void".into()),
                place: None,
                size: 0,
            },
            Operand::Register(i) => Value {
                ty: None,
                type_name: "int".into(),
                kind: ValueKind::UInt(u64::from(self.ctx.regs[i])),
                place: Some(Place::Register(i as u16)),
                size: 4,
            },
            Operand::Func { address, ret } => Value {
                ty: None,
                type_name: format!("{} ()", self.info.type_name(ret)),
                kind: ValueKind::Function(address),
                place: Some(Place::Address(address)),
                size: 0,
            },
            Operand::Address { target, address } => Value {
                ty: None,
                type_name: format!("{} *", self.info.type_name(target)),
                kind: ValueKind::Pointer {
                    address,
                    text: None,
                    target,
                },
                place: None,
                size: 4,
            },
            Operand::Typed { ty, num } => {
                // On passe par la lecture d'un emplacement calculé : même mise en forme que pour une variable.
                let size = self.info.size_of(ty).clamp(1, 8) as usize;
                let bytes = match num {
                    Num::I(n) => n.to_le_bytes()[..size].to_vec(),
                    Num::F(f) if size == 4 => (f as f32).to_le_bytes().to_vec(),
                    Num::F(f) => f.to_le_bytes()[..size].to_vec(),
                };
                let mut value = self.read(Some(ty), Place::Computed(bytes));
                value.place = None;
                value
            }
            Operand::Object { ty, place } => self.read(ty, place),
        })
    }
}

/// `value` réduit à `size` octets, avec ou sans signe.
fn truncate(value: i64, size: u32, signed: bool) -> i64 {
    if size == 0 || size >= 8 {
        return value;
    }
    let bits = size * 8;
    let masked = (value as u64) & ((1u64 << bits) - 1);
    if signed && masked >> (bits - 1) & 1 == 1 {
        (masked | (!0u64 << bits)) as i64
    } else {
        masked as i64
    }
}

impl DebugInfo {
    /// Est-ce un type entier (pour les conversions) ?
    pub fn is_integer(&self, id: TypeId) -> bool {
        matches!(
            self.ty(self.strip(id)).kind,
            TypeKind::Base(
                Encoding::Signed
                    | Encoding::Unsigned
                    | Encoding::SignedChar
                    | Encoding::UnsignedChar
                    | Encoding::Bool
            )
        )
    }
}

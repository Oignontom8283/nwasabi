//! Les expressions que l'on saisit pour regarder une variable : `x`, `shape.corner.x`, `*p`, `&n`, `a[2]`, `n + 1`.

use super::eval::{Context, Place, Value, ValueKind, locate, read_value};
use super::{DebugInfo, Encoding, TypeId, TypeKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalError(pub String);

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

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

const PUNCT: [&str; 25] = [
    "->", "<<", ">>", "<=", ">=", "==", "!=", "&&", "||", ".", "[", "]", "(", ")", "*", "&", "+",
    "-", "/", "%", "<", ">", "!", "~", ",",
];

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
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                if s.contains('.') {
                    out.push(Token::Float(
                        s.parse().map_err(|_| EvalError("nombre invalide".into()))?,
                    ));
                } else {
                    out.push(Token::Number(
                        s.parse().map_err(|_| EvalError("nombre invalide".into()))?,
                    ));
                }
            }
        } else if c.is_alphabetic() || c == '_' || c == '$' {
            let start = i;
            while i < chars.len()
                && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '$')
            {
                i += 1;
            }
            out.push(Token::Ident(chars[start..i].iter().collect()));
        } else {
            let rest: String = chars[i..].iter().take(2).collect();
            let p = PUNCT.iter().find(|p| rest.starts_with(**p));
            match p {
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

/// Une valeur en cours de calcul : un entier, un flottant, ou un objet en mémoire/registre avec son type.
#[derive(Debug, Clone)]
enum Operand {
    Int(i64),
    Float(f64),
    Object {
        ty: Option<TypeId>,
        place: Place,
    },
    /// Une adresse calculée (`&x`) avec le type pointé.
    Address {
        target: Option<TypeId>,
        address: u32,
    },
}

struct Parser<'a, 'm> {
    info: &'a DebugInfo,
    tokens: Vec<Token>,
    at: usize,
    pc: u32,
    ctx: &'a mut Context<'m>,
    /// Position du résultat dans l'expression : sert à ne lire qu'une fois à la fin.
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
    if let Some(expr) = frame_base
        && ctx.frame_base.is_none()
        && let Ok(Place::Register(r)) = locate(expr, ctx)
        && let Some(&v) = ctx.regs.get(usize::from(r))
    {
        ctx.frame_base = Some(v);
    } else if let Some(expr) = frame_base
        && ctx.frame_base.is_none()
        && let Ok(Place::Address(a)) = locate(expr, ctx)
    {
        ctx.frame_base = Some(a);
    }
    let mut p = Parser {
        info,
        tokens,
        at: 0,
        pc,
        ctx,
        steps: 0,
    };
    let result = p.expression(0)?;
    if p.at < p.tokens.len() {
        return fail("texte inattendu à la fin de l'expression");
    }
    p.finish(result)
}

fn precedence(op: &str) -> Option<u8> {
    Some(match op {
        "||" => 1,
        "&&" => 2,
        "==" | "!=" => 3,
        "<" | ">" | "<=" | ">=" => 4,
        "<<" | ">>" => 5,
        "+" | "-" => 6,
        "*" | "/" | "%" => 7,
        _ => return None,
    })
}

impl Parser<'_, '_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn eat(&mut self, punct: &str) -> bool {
        if matches!(self.peek(), Some(Token::Punct(p)) if *p == punct) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn expression(&mut self, min: u8) -> Result<Operand, EvalError> {
        let mut left = self.unary()?;
        while let Some(Token::Punct(op)) = self.peek().cloned() {
            let Some(prec) = precedence(op) else {
                break;
            };
            if prec < min.max(1) {
                break;
            }
            self.at += 1;
            let right = self.expression(prec + 1)?;
            left = self.binary(op, left, right)?;
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Operand, EvalError> {
        self.steps += 1;
        if self.steps > 512 {
            return fail("expression trop longue");
        }
        if self.eat("-") {
            return match self.unary()? {
                Operand::Float(f) => Ok(Operand::Float(-f)),
                other => Ok(Operand::Int(self.int_of(&other)?.wrapping_neg())),
            };
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
            let v = self.unary()?;
            return match v {
                Operand::Object {
                    ty,
                    place: Place::Address(address),
                } => Ok(Operand::Address {
                    target: ty,
                    address,
                }),
                _ => fail("on ne peut prendre l'adresse que d'un objet en mémoire"),
            };
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Result<Operand, EvalError> {
        let mut v = self.primary()?;
        loop {
            if self.eat(".") {
                let Some(Token::Ident(name)) = self.peek().cloned() else {
                    return fail("nom de champ attendu après « . »");
                };
                self.at += 1;
                v = self.field(v, &name)?;
            } else if self.eat("->") {
                let Some(Token::Ident(name)) = self.peek().cloned() else {
                    return fail("nom de champ attendu après « -> »");
                };
                self.at += 1;
                let target = self.deref(v)?;
                v = self.field(target, &name)?;
            } else if self.eat("[") {
                let index = self.expression(0)?;
                if !self.eat("]") {
                    return fail("« ] » attendu");
                }
                v = self.index(v, &index)?;
            } else {
                return Ok(v);
            }
        }
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
                let v = self.expression(0)?;
                if !self.eat(")") {
                    return fail("« ) » attendue");
                }
                Ok(v)
            }
            Some(Token::Ident(name)) => {
                self.at += 1;
                self.variable(&name)
            }
            Some(_) => fail("expression inattendue"),
            None => fail("expression incomplète"),
        }
    }

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
            return Ok(Operand::Int(i64::from(self.ctx.regs[index])));
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
            return Ok(Operand::Object { ty: var.ty, place });
        }
        if let Some(f) = self.info.function_named(name) {
            return Ok(Operand::Address {
                target: None,
                address: f.low,
            });
        }
        // Une constante d'énumération.
        for (i, t) in (0..self.info.type_count()).map(|i| (i, self.info.ty(i))) {
            if let TypeKind::Enum { variants, .. } = &t.kind
                && let Some((_, v)) = variants.iter().find(|(n, _)| n == name)
            {
                let _ = i;
                return Ok(Operand::Int(*v));
            }
        }
        fail(format!("symbole inconnu : {name}"))
    }

    fn read(&mut self, ty: Option<TypeId>, place: Place) -> Value {
        read_value(self.info, ty, place, self.ctx)
    }

    /// Le contenu d'un opérande, converti en entier.
    fn int_of(&mut self, v: &Operand) -> Result<i64, EvalError> {
        match v {
            Operand::Int(n) => Ok(*n),
            Operand::Float(f) => Ok(*f as i64),
            Operand::Address { address, .. } => Ok(i64::from(*address)),
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
            Operand::Object { ty, place } => match self.read(*ty, place.clone()).kind {
                ValueKind::Float(f) => Some(f),
                _ => None,
            },
            _ => None,
        }
    }

    fn deref(&mut self, v: Operand) -> Result<Operand, EvalError> {
        let (target, address) = match &v {
            Operand::Address { target, address } => (*target, *address),
            Operand::Object { ty, place } => {
                let value = self.read(*ty, place.clone());
                match value.kind {
                    ValueKind::Pointer {
                        address, target, ..
                    } => (target, address),
                    ValueKind::Array { .. } => match place {
                        Place::Address(a) => {
                            let elem =
                                ty.and_then(|t| match &self.info.ty(self.info.strip(t)).kind {
                                    TypeKind::Array { elem, .. } => Some(*elem),
                                    _ => None,
                                });
                            (elem, *a)
                        }
                        _ => return fail("tableau hors mémoire"),
                    },
                    _ => return fail("seul un pointeur peut être déréférencé"),
                }
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

    fn field(&mut self, v: Operand, name: &str) -> Result<Operand, EvalError> {
        let Operand::Object {
            ty: Some(ty),
            place,
        } = v
        else {
            return fail("ce n'est pas une structure");
        };
        let stripped = self.info.strip(ty);
        let TypeKind::Struct { fields, .. } = &self.info.ty(stripped).kind else {
            return fail("ce n'est pas une structure");
        };
        let Some(f) = fields.iter().find(|f| f.name == name) else {
            return fail(format!("pas de champ « {name} »"));
        };
        let Place::Address(base) = place else {
            return fail("structure hors mémoire");
        };
        Ok(Operand::Object {
            ty: Some(f.ty),
            place: Place::Address(base.wrapping_add(f.offset)),
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
                    TypeKind::Array { elem, counts } => {
                        let Place::Address(a) = place else {
                            return fail("tableau hors mémoire");
                        };
                        // Tableau à plusieurs dimensions : l'indice choisit une tranche.
                        if counts.len() > 1 {
                            return fail(
                                "tableaux à plusieurs dimensions : écrire a[i][j] n'est pas encore géré",
                            );
                        }
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

    fn binary(&mut self, op: &str, left: Operand, right: Operand) -> Result<Operand, EvalError> {
        // Arithmétique de pointeurs : `p + n` avance de `n` éléments.
        if matches!(op, "+" | "-")
            && let Operand::Address { target, address } = left
            && !matches!(right, Operand::Address { .. })
        {
            let n = self.int_of(&right)?;
            let size = target.map_or(1, |t| self.info.size_of(t).max(1));
            let delta = (n as i32).wrapping_mul(size as i32) as u32;
            let address = if op == "+" {
                address.wrapping_add(delta)
            } else {
                address.wrapping_sub(delta)
            };
            return Ok(Operand::Address { target, address });
        }
        if let (Some(a), Some(b)) = (
            self.float_of(&left),
            self.float_of(&right)
                .or_else(|| self.int_of(&right).ok().map(|n| n as f64)),
        ) {
            return Ok(Self::float_op(op, a, b));
        }
        if let (Operand::Int(_) | Operand::Object { .. }, Some(b)) = (&left, self.float_of(&right))
        {
            let a = self.int_of(&left)? as f64;
            return Ok(Self::float_op(op, a, b));
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

    fn float_op(op: &str, a: f64, b: f64) -> Operand {
        match op {
            "+" => Operand::Float(a + b),
            "-" => Operand::Float(a - b),
            "*" => Operand::Float(a * b),
            "/" => Operand::Float(a / b),
            "<" => Operand::Int(i64::from(a < b)),
            ">" => Operand::Int(i64::from(a > b)),
            "<=" => Operand::Int(i64::from(a <= b)),
            ">=" => Operand::Int(i64::from(a >= b)),
            "==" => Operand::Int(i64::from(a == b)),
            "!=" => Operand::Int(i64::from(a != b)),
            _ => Operand::Float(f64::NAN),
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
            Operand::Object { ty, place } => self.read(ty, place),
        })
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

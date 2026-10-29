//! Le débogage du code source : lignes, pile, variables et expressions, pas à la ligne, lus dans les informations DWARF
//! de `tests/apps/bin/debugme.nwa` (compilée avec `-O0 -g`, voir `tests/apps/src/debugme.c`).

use nwasabi::json::{self, Value, ValueExt};
use nwasabi::session::Session;

fn app(name: &str) -> String {
    let path = format!("{}/tests/apps/bin/{name}.nwa", env!("CARGO_MANIFEST_DIR"));
    json::base64_encode(&std::fs::read(path).unwrap())
}

fn cmd(name: &str) -> Value {
    Value::object().with("cmd", name)
}

fn call(session: &mut Session, request: Value) -> Value {
    let reply = session.handle(&request);
    assert_eq!(reply["ok"], true, "{request} -> {reply}");
    reply
}

fn debugme() -> Session {
    let mut session = Session::new();
    call(&mut session, cmd("load").with("nwa", app("debugme")));
    session
}

/// Pose un point d'arrêt à `fonction`, lance jusqu'à lui et rend la réponse de `run`.
fn run_to_function(session: &mut Session, name: &str) -> Value {
    call(session, cmd("break_function").with("name", name));
    call(session, cmd("run").with("instructions", 5_000_000u64))
}

fn eval(session: &mut Session, expr: &str) -> Value {
    call(session, cmd("eval").with("expr", expr))
}

fn text(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_owned()
}

#[test]
fn an_application_with_debug_info_lists_its_sources_and_functions() {
    let mut s = debugme();
    let info = call(&mut s, cmd("debug_info"));
    assert_eq!(info["available"], true);
    assert_eq!(info["files"][0]["path"], "src/debugme.c");
    assert_eq!(info["functions"], 4);
    assert!(info["globals"].as_u64().unwrap() >= 2);
}

#[test]
fn an_application_without_debug_info_says_so() {
    let mut s = Session::new();
    call(&mut s, cmd("load").with("nwa", app("integer")));
    assert_eq!(call(&mut s, cmd("debug_info"))["available"], false);
    let reply = s.handle(&cmd("frames"));
    assert_eq!(reply["ok"], false);
    assert!(text(&reply, "error").contains("-g"), "{reply}");
}

#[test]
fn addresses_map_to_lines_and_back() {
    let mut s = debugme();
    let at = call(&mut s, cmd("break_function").with("name", "add"));
    assert_eq!(at["function"], "add");
    assert_eq!(
        at["line"], 15,
        "après le prologue : la première ligne du corps"
    );
    let found = call(
        &mut s,
        cmd("line_at").with("address", at["address"].clone()),
    );
    assert_eq!(found["line"], 15);

    // Une ligne sans code (le commentaire, ligne 1) se pose sur la première ligne qui en a.
    let b = call(
        &mut s,
        cmd("break_line")
            .with("file", "debugme.c")
            .with("line", 12u64),
    );
    assert_eq!(
        b["line"], 14,
        "la déclaration de `add` est la première ligne avec du code"
    );
    let lines = call(&mut s, cmd("source_lines").with("file", "src/debugme.c"));
    let list: Vec<u64> = lines["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l.as_u64().unwrap())
        .collect();
    assert!(
        list.contains(&15) && list.contains(&44) && !list.contains(&13),
        "{list:?}"
    );
}

#[test]
fn a_breakpoint_shows_the_stack_the_parameters_and_the_locals() {
    let mut s = debugme();
    let stop = run_to_function(&mut s, "factorial");
    assert_eq!(stop["stop"]["kind"], "breakpoint");
    let frames = call(&mut s, cmd("frames"));
    let names: Vec<String> = frames["frames"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| text(f, "function"))
        .collect();
    assert_eq!(&names[..2], ["factorial", "main"], "{frames}");
    assert_eq!(
        frames["frames"][1]["line"], 40,
        "la ligne de l'appel dans main"
    );

    let locals = call(&mut s, cmd("locals"));
    assert_eq!(locals["params"][0]["name"], "n");
    assert_eq!(locals["params"][0]["value"], "5");
    assert_eq!(locals["locals"][0]["name"], "rest");
}

#[test]
fn the_stack_of_a_recursion_is_exact() {
    let mut s = debugme();
    call(&mut s, cmd("break_function").with("name", "factorial"));
    // Quatre arrêts de plus : n vaut 5, 4, 3, 2 puis 1 ; le cadre le plus récent montre toujours le n courant.
    for n in (1..=5).rev() {
        let stop = call(&mut s, cmd("run").with("instructions", 5_000_000u64));
        assert_eq!(stop["stop"]["kind"], "breakpoint");
        assert_eq!(eval(&mut s, "n")["value"], n.to_string());
        let frames = call(&mut s, cmd("frames"));
        let functions: Vec<String> = frames["frames"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| text(f, "function"))
            .collect();
        let depth = 5 - n + 1;
        assert!(
            functions[..depth].iter().all(|f| f == "factorial"),
            "{functions:?}"
        );
        assert_eq!(functions[depth], "main", "{functions:?}");
    }
    // Les variables d'un cadre appelant se lisent avec `frame` : le n de l'appel précédent.
    let up = call(&mut s, cmd("eval").with("expr", "n").with("frame", 1u64));
    assert_eq!(up["value"], "2");
}

#[test]
fn expressions_read_structures_arrays_pointers_and_enumerations() {
    let mut s = debugme();
    run_to_function(&mut s, "fill");
    // `fill(&shape, 10)` : le paramètre pointe sur la structure de main.
    assert_eq!(eval(&mut s, "base")["value"], "10");
    assert_eq!(eval(&mut s, "shape->corner.y")["value"], "2");
    assert_eq!(eval(&mut s, "shape->color")["value"], "GREEN");
    let name = eval(&mut s, "shape->name");
    assert!(text(&name, "value").ends_with("\"box\""), "{name}");
    assert_eq!(eval(&mut s, "shape->corner.x + base * 2")["value"], "21");
    let whole = eval(&mut s, "*shape");
    assert_eq!(whole["type"], "Shape");
    assert_eq!(whole["children"].as_array().unwrap().len(), 4);
    assert_eq!(eval(&mut s, "origin")["value"], "{x = 3, y = -4}");
    assert_eq!(eval(&mut s, "global_counter")["value"], "7");
    assert_eq!(eval(&mut s, "&origin")["type"], "Point *");

    // Un tableau se remplit au fil de la fonction : on regarde ses éléments après la boucle.
    call(
        &mut s,
        cmd("break_line")
            .with("file", "debugme.c")
            .with("line", 33u64),
    );
    call(&mut s, cmd("run").with("instructions", 5_000_000u64));
    let flags = eval(&mut s, "shape->flags");
    assert_eq!(flags["value"], "{10, 11, 12}");
    assert_eq!(flags["count"], 3);
    assert_eq!(eval(&mut s, "shape->flags[2]")["value"], "12");
    assert_eq!(eval(&mut s, "total")["value"], "33");
}

#[test]
fn bad_expressions_are_explained() {
    let mut s = debugme();
    run_to_function(&mut s, "fill");
    for (expr, expected) in [
        ("nonexistent", "symbole inconnu"),
        ("shape->nothing", "pas de champ"),
        ("base.x", "pas une structure"),
        ("1 +", "incomplète"),
        ("base / 0", "division par zéro"),
        ("$nothing", "registre inconnu"),
    ] {
        let reply = s.handle(&cmd("eval").with("expr", expr));
        assert_eq!(reply["ok"], false, "{expr}");
        assert!(
            text(&reply, "error").contains(expected),
            "{expr} -> {reply}"
        );
    }
}

#[test]
fn assigning_changes_the_variable() {
    let mut s = debugme();
    run_to_function(&mut s, "fill");
    call(
        &mut s,
        cmd("assign").with("expr", "base").with("value", 20u64),
    );
    call(
        &mut s,
        cmd("break_line")
            .with("file", "debugme.c")
            .with("line", 33u64),
    );
    call(&mut s, cmd("run").with("instructions", 5_000_000u64));
    // fill écrit base, base+1, base+2 : 20 + 21 + 22.
    assert_eq!(eval(&mut s, "total")["value"], "63");
    call(
        &mut s,
        cmd("assign")
            .with("expr", "global_counter")
            .with("value", -5i64),
    );
    assert_eq!(eval(&mut s, "global_counter")["value"], "-5");
}

#[test]
fn ptype_describes_types() {
    let mut s = debugme();
    run_to_function(&mut s, "fill");
    let shape = call(&mut s, cmd("ptype").with("expr", "Shape"));
    let t = text(&shape, "type");
    assert!(
        t.contains("Point corner") && t.contains("uint8_t [3] flags") && t.contains("16 octets"),
        "{t}"
    );
    let color = call(&mut s, cmd("ptype").with("expr", "Color"));
    assert!(text(&color, "type").contains("GREEN = 5"), "{color}");
    let of_value = call(&mut s, cmd("ptype").with("expr", "*shape"));
    assert!(text(&of_value, "type").contains("struct"), "{of_value}");
}

#[test]
fn stepping_by_line_goes_into_over_and_out() {
    let mut s = debugme();
    // Au début de main, ligne 38 (la structure) : on avance ligne à ligne.
    run_to_function(&mut s, "main");
    let here = call(&mut s, cmd("frames"));
    assert_eq!(here["frames"][0]["function"], "main");
    let first = here["frames"][0]["line"].as_u64().unwrap();

    // « over » reste dans main et avance d'une ligne de source.
    let next = call(&mut s, cmd("step_source").with("mode", "over"));
    assert_eq!(next["function"], "main");
    assert!(next["line"].as_u64().unwrap() > first, "{next}");

    // On arrive sur la ligne 39, `CHECK(add(2, 3) == 5)` ; « into » entre dans add.
    let mut guard = 0;
    loop {
        let at = call(&mut s, cmd("frames"));
        if at["frames"][0]["line"] == 39 || guard > 6 {
            break;
        }
        call(&mut s, cmd("step_source").with("mode", "over"));
        guard += 1;
    }
    let inside = call(&mut s, cmd("step_source").with("mode", "into"));
    assert_eq!(inside["function"], "add", "{inside}");
    assert_eq!(inside["line"], 15);
    assert_eq!(eval(&mut s, "a + b")["value"], "5");

    // « out » ramène dans main, juste après l'appel.
    let out = call(&mut s, cmd("step_source").with("mode", "out"));
    assert_eq!(out["function"], "main", "{out}");
    assert_eq!(
        out["line"], 39,
        "on revient au milieu de la ligne de l'appel"
    );
}

#[test]
fn stepping_over_a_call_runs_it_whole() {
    let mut s = debugme();
    call(
        &mut s,
        cmd("break_line")
            .with("file", "debugme.c")
            .with("line", 41u64),
    );
    call(&mut s, cmd("run").with("instructions", 5_000_000u64));
    let frames = call(&mut s, cmd("frames"));
    assert_eq!(frames["frames"][0]["line"], 41);
    // La ligne 41 appelle factorial(5) : « over » ne s'y arrête pas.
    let next = call(&mut s, cmd("step_source").with("mode", "over"));
    assert_eq!(next["function"], "main");
    assert_eq!(next["line"], 42);
}

#[test]
fn a_program_runs_to_the_end_with_the_debugger_attached() {
    let mut s = debugme();
    let reply = call(&mut s, cmd("run").with("instructions", 5_000_000u64));
    assert_eq!(reply["stop"]["kind"], "exited", "{reply}");
    assert_eq!(
        reply["stop"]["code"], 0,
        "toutes les vérifications de l'application ont réussi"
    );
}

#[test]
fn the_linked_application_exports_as_an_elf_for_gdb() {
    let mut s = debugme();
    let reply = call(&mut s, cmd("export_elf"));
    let elf = json::base64_decode(reply["elf"].as_str().unwrap()).unwrap();
    assert_eq!(&elf[..4], b"\x7fELF");
    assert_eq!(u16::from_le_bytes([elf[16], elf[17]]), 2, "ET_EXEC");
    assert_eq!(u16::from_le_bytes([elf[18], elf[19]]), 40, "EM_ARM");
    let contains = |needle: &str| elf.windows(needle.len()).any(|w| w == needle.as_bytes());
    for name in [
        "factorial",
        "global_counter",
        ".debug_info",
        ".debug_line",
        ".debug_frame",
        ".symtab",
    ] {
        assert!(contains(name), "{name} absent de l'ELF");
    }
}

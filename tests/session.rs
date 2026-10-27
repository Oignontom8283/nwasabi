//! Le protocole de session JSON, de bout en bout (sans passer par un binaire).

mod support;

use nwasabi::json::{self, Value};
use nwasabi::session::Session;

fn ask(session: &mut Session, request: &str) -> Value {
    let reply = session.handle_line(request);
    json::parse(&reply).unwrap_or_else(|e| panic!("réponse invalide ({e}) : {reply}"))
}

fn ok(session: &mut Session, request: &str) -> Value {
    let reply = ask(session, request);
    assert_eq!(
        reply.get("ok"),
        Some(&Value::Bool(true)),
        "{request} -> {reply}"
    );
    reply
}

fn error(session: &mut Session, request: &str) -> String {
    let reply = ask(session, request);
    assert_eq!(
        reply.get("ok"),
        Some(&Value::Bool(false)),
        "{request} -> {reply}"
    );
    reply
        .get("error")
        .and_then(Value::as_str)
        .unwrap()
        .to_owned()
}

#[test]
fn errors_are_replies_not_failures_and_ids_are_echoed() {
    let mut s = Session::new();
    assert!(error(&mut s, "pas du json").contains("JSON invalide"));
    assert!(error(&mut s, r#"{"cmd":"inconnue"}"#).contains("commande inconnue"));
    assert!(error(&mut s, r#"{"cmd":"fs_list"}"#).contains("aucun appareil"));
    assert!(error(&mut s, r#"{"nocmd":1}"#).contains("cmd"));
    let r = ask(&mut s, r#"{"cmd":"help","id":"abc"}"#);
    assert_eq!(r.get("id").and_then(Value::as_str), Some("abc"));
    assert!(r.get("commands").and_then(Value::as_array).unwrap().len() > 20);
}

#[test]
fn files_round_trip_through_the_protocol() {
    let mut s = Session::new();
    ok(&mut s, r#"{"cmd":"new","model":"n0110"}"#);
    ok(
        &mut s,
        r#"{"cmd":"fs_write","name":"a.py","python":true,"text":"print(1)","auto_import":true}"#,
    );
    ok(
        &mut s,
        r#"{"cmd":"fs_write","name":"b.txt","content":"AAEC"}"#,
    );
    let list = ok(&mut s, r#"{"cmd":"fs_list"}"#);
    let files = list.get("files").and_then(Value::as_array).unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].get("kind").and_then(Value::as_str), Some("python"));
    assert_eq!(files[1].get("size").and_then(Value::as_i64), Some(3));

    let r = ok(&mut s, r#"{"cmd":"fs_read","name":"a.py","python":true}"#);
    assert_eq!(r.get("text").and_then(Value::as_str), Some("print(1)"));
    assert_eq!(r.get("auto_import"), Some(&Value::Bool(true)));
    let r = ok(&mut s, r#"{"cmd":"fs_read","name":"b.txt"}"#);
    assert_eq!(r.get("content").and_then(Value::as_str), Some("AAEC"));

    ok(&mut s, r#"{"cmd":"fs_rename","from":"b.txt","to":"c.txt"}"#);
    ok(&mut s, r#"{"cmd":"fs_remove","name":"a.py"}"#);
    assert!(error(&mut s, r#"{"cmd":"fs_remove","name":"a.py"}"#).contains("introuvable"));
    let stats = ok(&mut s, r#"{"cmd":"fs_stats"}"#);
    assert_eq!(stats.get("records").and_then(Value::as_i64), Some(1));
    assert_eq!(stats.get("total").and_then(Value::as_i64), Some(43008));
}

#[test]
fn settings_are_visible_in_the_memory_structures() {
    let mut s = Session::new();
    ok(&mut s, r#"{"cmd":"new"}"#);
    ok(
        &mut s,
        r#"{"cmd":"settings_set","name":"version","value":"27.0.1"}"#,
    );
    ok(
        &mut s,
        r#"{"cmd":"settings_set","name":"device_name","value":"Mon appareil"}"#,
    );
    ok(
        &mut s,
        r#"{"cmd":"settings_set","name":"battery","value":{"level":1,"voltage":3.5,"charging":true}}"#,
    );
    let inspect = ok(&mut s, r#"{"cmd":"inspect"}"#);
    let st = inspect.get("structures").unwrap();
    let kernel = st.get("kernel_header").unwrap();
    assert_eq!(
        kernel.get("software_version").and_then(Value::as_str),
        Some("27.0.1")
    );
    assert_eq!(kernel.get("valid"), Some(&Value::Bool(true)));
    assert_eq!(
        st.get("device_name").and_then(Value::as_str),
        Some("Mon appareil")
    );

    let got = ok(&mut s, r#"{"cmd":"settings_get"}"#);
    assert_eq!(
        got.get("battery")
            .and_then(|b| b.get("level"))
            .and_then(Value::as_i64),
        Some(1)
    );
    assert!(
        error(
            &mut s,
            r#"{"cmd":"settings_set","name":"serial_number","value":"court"}"#
        )
        .contains("serial_number")
    );
    assert!(error(&mut s, r#"{"cmd":"settings_set","name":"zzz","value":1}"#).contains("inconnu"));
}

#[test]
fn memory_can_be_read_written_and_followed() {
    let mut s = Session::new();
    ok(&mut s, r#"{"cmd":"new"}"#);
    let cursor = ok(&mut s, r#"{"cmd":"changes","cursor":1000000}"#)
        .get("cursor")
        .and_then(Value::as_i64)
        .unwrap();
    ok(
        &mut s,
        r#"{"cmd":"memory_write","address":"0x20020000","hex":"deadbeef"}"#,
    );
    let r = ok(
        &mut s,
        r#"{"cmd":"memory_read","address":537001984,"length":4}"#,
    );
    assert_eq!(r.get("hex").and_then(Value::as_str), Some("deadbeef"));
    let changes = ok(&mut s, &format!(r#"{{"cmd":"changes","cursor":{cursor}}}"#));
    assert_eq!(
        changes
            .get("ranges")
            .and_then(Value::as_array)
            .unwrap()
            .len(),
        1
    );
    assert!(
        error(&mut s, r#"{"cmd":"memory_read","address":0,"length":4}"#)
            .contains("hors de toute zone")
    );
    assert!(
        error(
            &mut s,
            r#"{"cmd":"memory_read","address":0,"length":99999999}"#
        )
        .contains("limitée")
    );
    let regions = ok(&mut s, r#"{"cmd":"regions"}"#);
    assert!(
        regions
            .get("regions")
            .and_then(Value::as_array)
            .unwrap()
            .len()
            >= 3
    );
}

#[test]
fn running_without_an_app_is_an_error() {
    let mut s = Session::new();
    ok(&mut s, r#"{"cmd":"new"}"#);
    assert!(error(&mut s, r#"{"cmd":"run"}"#).contains("aucune application"));
}

#[test]
fn the_session_draws_the_same_screen_as_the_corpus_reference() {
    let Some(apps) = support::corpus() else {
        return support::skip_without_corpus();
    };
    let (_, nwa) = apps
        .iter()
        .find(|(n, _)| n.starts_with("sierpinski"))
        .unwrap();
    let mut s = Session::new();
    let load = format!(
        r#"{{"cmd":"load","model":"n0110","nwa":"{}"}}"#,
        json::base64_encode(nwa)
    );
    let loaded = ok(&mut s, &load);
    assert!(loaded.get("app").and_then(|a| a.get("name")).is_some());

    let ran = ok(&mut s, r#"{"cmd":"run","instructions":40000000}"#);
    assert_eq!(
        ran.get("stop")
            .and_then(|s| s.get("kind"))
            .and_then(Value::as_str),
        Some("budget")
    );
    let screen = ok(&mut s, r#"{"cmd":"screen","rgba":true}"#);
    // Même empreinte que dans tests/golden/screens.tsv (lancement par `emulator_corpus`).
    assert_eq!(
        screen.get("crc32").and_then(Value::as_str),
        Some("a6a4533d")
    );
    let rgba = json::base64_decode(screen.get("rgba").and_then(Value::as_str).unwrap()).unwrap();
    assert_eq!(rgba.len(), 320 * 240 * 4);
    // Le PNG est l'affaire de l'hôte : le cœur le refuse.
    assert!(error(&mut s, r#"{"cmd":"screen","png":true}"#).contains("côté hôte"));

    let usage = ok(&mut s, r#"{"cmd":"usage"}"#);
    assert!(
        usage
            .get("stack_low_water")
            .and_then(Value::as_i64)
            .is_some()
    );
    let cpu = ok(&mut s, r#"{"cmd":"cpu"}"#);
    assert_eq!(cpu.get("r").and_then(Value::as_array).unwrap().len(), 13);
    let svcs = ok(&mut s, r#"{"cmd":"svc_stats"}"#);
    assert!(
        !svcs
            .get("counts")
            .and_then(Value::as_array)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn keys_reach_the_app_and_waiting_is_reported() {
    let Some(apps) = support::corpus() else {
        return support::skip_without_corpus();
    };
    let (_, nwa) = apps
        .iter()
        .find(|(n, _)| n.starts_with("mandelbrot"))
        .unwrap();
    let mut s = Session::new();
    ok(
        &mut s,
        &format!(r#"{{"cmd":"load","nwa":"{}"}}"#, json::base64_encode(nwa)),
    );
    let ran = ok(&mut s, r#"{"cmd":"run","instructions":100000000}"#);
    let kind = ran
        .get("stop")
        .and_then(|s| s.get("kind"))
        .and_then(Value::as_str);
    assert_eq!(kind, Some("wait_event"), "{ran}");
    assert!(error(&mut s, r#"{"cmd":"key","key":"nope"}"#).contains("touche inconnue"));
    ok(&mut s, r#"{"cmd":"key","key":"ok","action":"press"}"#);
    let ran = ok(&mut s, r#"{"cmd":"run","instructions":1000000}"#);
    assert_ne!(
        ran.get("stop")
            .and_then(|s| s.get("kind"))
            .and_then(Value::as_str),
        Some("fault"),
        "{ran}"
    );
}

/// Joue le début d'une partie de démineur et rend l'écran (RGB565, base64 décodé).
fn play_minesweeper(firmware: &str) -> Option<Vec<u8>> {
    let apps = support::corpus()?;
    let (_, nwa) = apps
        .iter()
        .find(|(n, _)| n.starts_with("minesweeper-1.3.5"))?;
    let mut s = Session::new();
    ok(
        &mut s,
        &format!(
            r#"{{"cmd":"load","model":"n0110","firmware":"{firmware}","nwa":"{}"}}"#,
            json::base64_encode(nwa)
        ),
    );
    ok(&mut s, r#"{"cmd":"run","instructions":20000000}"#);
    for key in ["ok", "ok", "down", "ok", "ok"] {
        ok(
            &mut s,
            &format!(r#"{{"cmd":"key","key":"{key}","action":"press"}}"#),
        );
        ok(&mut s, r#"{"cmd":"run","instructions":3000000}"#);
        ok(
            &mut s,
            &format!(r#"{{"cmd":"key","key":"{key}","action":"release"}}"#),
        );
        ok(&mut s, r#"{"cmd":"run","instructions":3000000}"#);
    }
    let screen = ok(&mut s, r#"{"cmd":"screen","rgb565":true}"#);
    json::base64_decode(screen.get("rgb565").and_then(Value::as_str).unwrap())
}

fn pixel(screen: &[u8], x: usize, y: usize) -> u16 {
    let i = (y * 320 + x) * 2;
    u16::from_le_bytes([screen[i], screen[i + 1]])
}

#[test]
fn eadkp_apps_find_their_storage_with_the_default_firmware() {
    // Le démineur (écrit avec eadkp) devine le modèle d'après la flash : avec la disposition des
    // appareils relevés il trouve le système de fichiers ; la partie se joue.
    let Some(screen) = play_minesweeper("24") else {
        return support::skip_without_corpus();
    };
    assert_ne!(
        pixel(&screen, 160, 200),
        0xf800,
        "l'écran n'est pas celui d'une panique"
    );
}

#[test]
fn the_26_4_0_layout_fools_the_model_detection_of_eadkp_apps() {
    // Reproduit le comportement connu (voir la documentation du profil) : avec l'en-tête du
    // userland à 0x90020000 sur N0110, l'application croit être sur N0120 et son système de
    // fichiers lui paraît invalide. Elle affiche l'écran rouge de sa panique.
    let Some(screen) = play_minesweeper("26.4.0") else {
        return support::skip_without_corpus();
    };
    assert_eq!(pixel(&screen, 160, 200), 0xf800);
}

#[test]
fn an_unknown_firmware_is_refused() {
    let mut s = Session::new();
    assert!(error(&mut s, r#"{"cmd":"new","firmware":"1.0"}"#).contains("firmware inconnu"));
}

fn time_ms(reply: &Value) -> i64 {
    reply.get("time_ms").and_then(Value::as_i64).unwrap()
}

fn stop_kind(reply: &Value) -> &str {
    reply
        .get("stop")
        .and_then(|s| s.get("kind"))
        .and_then(Value::as_str)
        .unwrap()
}

#[test]
fn with_the_host_clock_time_only_moves_when_the_host_says_so() {
    let Some(apps) = support::corpus() else {
        return support::skip_without_corpus();
    };
    // compressor attend 10 ms (msleep) entre deux lectures du clavier.
    let (_, nwa) = apps
        .iter()
        .find(|(n, _)| n.starts_with("compressor-1.0.0"))
        .unwrap();
    let mut s = Session::new();
    ok(
        &mut s,
        &format!(
            r#"{{"cmd":"load","clock":"host","nwa":"{}"}}"#,
            json::base64_encode(nwa)
        ),
    );
    let first = ok(&mut s, r#"{"cmd":"run","instructions":10000000}"#);
    assert_eq!(stop_kind(&first), "sleep", "{first}");
    assert_eq!(time_ms(&first), 0);

    // « Pause » : l'hôte relance l'exécution sans faire passer de temps, même très longtemps. Rien ne
    // se passe pour l'application, qui reste à dormir sans que son horloge bouge.
    for _ in 0..20 {
        let again = ok(&mut s, r#"{"cmd":"run","instructions":50000000}"#);
        assert_eq!(stop_kind(&again), "sleep");
        assert_eq!(time_ms(&again), 0);
    }

    // Le temps de l'hôte avance : l'application se réveille, puis se rendort.
    ok(&mut s, r#"{"cmd":"advance","ms":10}"#);
    let woken = ok(&mut s, r#"{"cmd":"run","instructions":10000000}"#);
    assert_eq!(stop_kind(&woken), "sleep");
    assert_eq!(time_ms(&woken), 10);
    // Les microsecondes sont acceptées.
    ok(&mut s, r#"{"cmd":"advance","us":2500}"#);
    assert_eq!(
        time_ms(&ok(&mut s, r#"{"cmd":"run","instructions":1000}"#)),
        12
    );
}

#[test]
fn with_the_virtual_clock_sleeping_jumps_the_time() {
    let Some(apps) = support::corpus() else {
        return support::skip_without_corpus();
    };
    let (_, nwa) = apps
        .iter()
        .find(|(n, _)| n.starts_with("compressor-1.0.0"))
        .unwrap();
    let mut s = Session::new();
    ok(
        &mut s,
        &format!(r#"{{"cmd":"load","nwa":"{}"}}"#, json::base64_encode(nwa)),
    );
    let ran = ok(&mut s, r#"{"cmd":"run","instructions":20000000}"#);
    assert!(time_ms(&ran) >= 100, "le temps virtuel avance seul : {ran}");
}

#[test]
fn an_unknown_clock_is_refused() {
    let mut s = Session::new();
    assert!(error(&mut s, r#"{"cmd":"new","clock":"lune"}"#).contains("horloge inconnue"));
}

#[test]
fn the_simulation_time_is_readable_at_any_moment_and_stops_without_the_host() {
    let Some(apps) = support::corpus() else {
        return support::skip_without_corpus();
    };
    let (_, nwa) = apps
        .iter()
        .find(|(n, _)| n.starts_with("compressor-1.0.0"))
        .unwrap();
    let mut s = Session::new();
    ok(
        &mut s,
        &format!(
            r#"{{"cmd":"load","clock":"host","nwa":"{}"}}"#,
            json::base64_encode(nwa)
        ),
    );
    let time = |s: &mut Session| {
        let r = ok(s, r#"{"cmd":"time"}"#);
        (
            r.get("time_us").and_then(Value::as_i64).unwrap(),
            r.get("time_ms").and_then(Value::as_i64).unwrap(),
        )
    };
    assert_eq!(time(&mut s), (0, 0));
    // L'application tourne, dort… : sans temps donné par l'hôte, le temps de simulation ne bouge pas.
    ok(&mut s, r#"{"cmd":"run","instructions":10000000}"#);
    assert_eq!(time(&mut s), (0, 0));
    // Le temps défile quand l'hôte le fait passer, même si l'application dort tout ce temps.
    ok(&mut s, r#"{"cmd":"advance","us":1500}"#);
    assert_eq!(time(&mut s), (1500, 1));
    ok(&mut s, r#"{"cmd":"advance","ms":20}"#);
    assert_eq!(time(&mut s), (21_500, 21));
    // C'est bien le temps que voit l'application : la réponse de `run` donne la même valeur.
    let ran = ok(&mut s, r#"{"cmd":"run","instructions":1000}"#);
    assert_eq!(ran.get("time_ms").and_then(Value::as_i64), Some(21));
}

#[test]
fn the_cpu_speed_is_chosen_per_device_and_can_change_later() {
    let mut s = Session::new();
    // Par défaut : la fréquence du modèle ; la fréquence personnalisée vaut celle du N0110.
    let r = ok(&mut s, r#"{"cmd":"new","model":"n0120"}"#);
    assert_eq!(r.get("speed").and_then(Value::as_str), Some("model"));
    assert_eq!(r.get("mhz").and_then(Value::as_u64), Some(192));
    assert_eq!(r.get("cpu_mhz").and_then(Value::as_u64), Some(540));
    assert_eq!(r.get("limit_mhz").and_then(Value::as_u64), Some(540));

    // Une fréquence donnée sans changer de mode est mémorisée mais ignorée.
    let r = ok(&mut s, r#"{"cmd":"speed","mhz":50}"#);
    assert_eq!(r.get("speed").and_then(Value::as_str), Some("model"));
    assert_eq!(r.get("mhz").and_then(Value::as_u64), Some(50));
    assert_eq!(r.get("cpu_mhz").and_then(Value::as_u64), Some(540));

    let r = ok(&mut s, r#"{"cmd":"speed","speed":"custom"}"#);
    assert_eq!(r.get("cpu_mhz").and_then(Value::as_u64), Some(50));
    assert_eq!(r.get("limit_mhz").and_then(Value::as_u64), Some(50));

    // Sans limite : rien à appliquer, le temps virtuel garde la fréquence du modèle.
    let r = ok(&mut s, r#"{"cmd":"speed","speed":"max"}"#);
    assert_eq!(r.get("limit_mhz"), Some(&Value::Null));
    assert_eq!(r.get("cpu_mhz").and_then(Value::as_u64), Some(540));

    // Sans champ, la commande rend l'état ; une nouvelle requête de création peut tout régler.
    let r = ok(&mut s, r#"{"cmd":"speed"}"#);
    assert_eq!(r.get("speed").and_then(Value::as_str), Some("max"));
    let r = ok(
        &mut s,
        r#"{"cmd":"new","model":"n0110","speed":"custom","mhz":100}"#,
    );
    assert_eq!(r.get("cpu_mhz").and_then(Value::as_u64), Some(100));
}

#[test]
fn bad_speeds_are_errors() {
    let mut s = Session::new();
    ok(&mut s, r#"{"cmd":"new"}"#);
    assert!(error(&mut s, r#"{"cmd":"speed","speed":"turbo"}"#).contains("vitesse inconnue"));
    assert!(error(&mut s, r#"{"cmd":"speed","mhz":0}"#).contains("mhz"));
    assert!(error(&mut s, r#"{"cmd":"speed","mhz":"vite"}"#).contains("mhz"));
    assert!(error(&mut s, r#"{"cmd":"speed","mhz":1000000}"#).contains("entre 1 et"));
    // La commande a besoin d'un appareil.
    let mut vide = Session::new();
    assert!(!error(&mut vide, r#"{"cmd":"speed"}"#).is_empty());
}

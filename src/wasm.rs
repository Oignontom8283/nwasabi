//! Interface WebAssembly : quelques fonctions `extern "C"` que JavaScript appelle.
//!
//! Le module n'a pas besoin de `wasm-bindgen` : l'échange se fait par la mémoire du module.
//! JavaScript réserve un tampon (`nwasabi_alloc`), y écrit une requête JSON en UTF-8, appelle
//! `nwasabi_request`, puis lit la réponse à `nwasabi_response_ptr` sur la longueur rendue. Les
//! requêtes et réponses sont celles de [`Session`](crate::session::Session). L'écran se lit sans
//! JSON, directement en mémoire, avec `nwasabi_framebuffer_ptr`.
//!
//! Un module WebAssembly ne tourne que dans un seul fil : l'état est un `thread_local`.

#![allow(unsafe_code)]

use std::cell::RefCell;

use crate::session::Session;

struct State {
    session: Session,
    response: Vec<u8>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State {
        session: Session::new(),
        response: Vec::new(),
    });
}

/// Réserve `len` octets dans la mémoire du module et rend leur adresse (à libérer avec
/// [`nwasabi_free`]).
#[unsafe(no_mangle)]
pub extern "C" fn nwasabi_alloc(len: usize) -> *mut u8 {
    let mut buffer = Vec::<u8>::with_capacity(len.max(1));
    let pointer = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    pointer
}

/// Libère un tampon obtenu par [`nwasabi_alloc`] avec la même taille.
///
/// # Safety
/// `ptr` doit venir de `nwasabi_alloc(len)` et ne pas avoir été libéré.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nwasabi_free(ptr: *mut u8, len: usize) {
    // SAFETY : garanti par l'appelant ; la capacité réservée était `len.max(1)`.
    drop(unsafe { Vec::from_raw_parts(ptr, 0, len.max(1)) });
}

/// Traite la requête JSON de `len` octets écrite à `ptr` et rend la longueur de la réponse.
///
/// # Safety
/// `ptr` doit désigner `len` octets lisibles (un tampon de `nwasabi_alloc`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nwasabi_request(ptr: *const u8, len: usize) -> usize {
    // SAFETY : garanti par l'appelant.
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    let reply = match std::str::from_utf8(bytes) {
        Ok(line) => STATE.with(|s| s.borrow_mut().session.handle_line(line)),
        Err(_) => "{\"ok\":false,\"error\":\"requête non UTF-8\"}".to_owned(),
    };
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        state.response = reply.into_bytes();
        state.response.len()
    })
}

/// Adresse de la dernière réponse (valable jusqu'à la requête suivante).
#[unsafe(no_mangle)]
pub extern "C" fn nwasabi_response_ptr() -> *const u8 {
    STATE.with(|s| s.borrow().response.as_ptr())
}

/// Adresse des pixels de l'écran (RGB565, `nwasabi_framebuffer_len` valeurs de 16 bits), ou 0 s'il
/// n'y a pas d'appareil. L'adresse change quand un nouvel appareil est créé (`new`, `load`).
#[unsafe(no_mangle)]
pub extern "C" fn nwasabi_framebuffer_ptr() -> *const u16 {
    STATE.with(|s| {
        s.borrow()
            .session
            .framebuffer()
            .map_or(std::ptr::null(), <[u16]>::as_ptr)
    })
}

/// Nombre de pixels de l'écran.
#[unsafe(no_mangle)]
pub extern "C" fn nwasabi_framebuffer_len() -> usize {
    (crate::kernel::display::WIDTH * crate::kernel::display::HEIGHT) as usize
}

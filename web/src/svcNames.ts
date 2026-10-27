// Noms des appels système du noyau d'Epsilon (et des appels privés de nwasabi).
export const NAMES: Record<number, string> = {
  1: "backlight_brightness", 2: "backlight_set_brightness", 3: "battery_is_charging", 4: "battery_level",
  5: "battery_voltage", 18: "display_pull_rect", 19: "display_push_rect", 20: "display_push_rect_uniform",
  21: "display_wait_for_vblank", 22: "events_copy_text", 23: "events_get_event", 24: "events_is_defined", 29: "fcc_id",
  33: "keyboard_pop_state", 34: "keyboard_scan", 39: "pcb_version", 45: "random", 47: "serial_number_copy",
  48: "timing_millis", 49: "timing_msleep", 50: "timing_usleep", 52: "usb_is_plugged", 55: "events_repeat_count",
  56: "compilation_flags", 128: "draw_string (nwasabi)", 129: "_sbrk (nwasabi)", 130: "_write (nwasabi)",
  137: "exit (nwasabi)",
};

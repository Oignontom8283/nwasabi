// L'enregistreur GIF : en-tête, dimensions, images fusionnées (deux images identiques à la suite n'en font qu'une, plus longue) et durées.
import { describe, expect, it } from "vitest";
import { GifRecorder } from "../src/gif.js";

const frame = (colors: number[][]) => Uint8ClampedArray.from(colors.flatMap((c) => [...c, 255]));
const red = [255, 0, 0];
const blue = [0, 0, 255];
const white = [255, 255, 255];

/** Les durées (en centièmes de seconde) des images, lues dans les extensions de contrôle (0x21 0xf9). */
function delays(gif: Uint8Array): number[] {
  const out: number[] = [];
  for (let i = 0; i < gif.length - 6; i++) if (gif[i] === 0x21 && gif[i + 1] === 0xf9 && gif[i + 2] === 4) out.push((gif[i + 4] as number) | ((gif[i + 5] as number) << 8));
  return out;
}

describe("enregistreur de GIF", () => {
  const w = 4;
  const h = 2;
  const recorder = new GifRecorder(w, h, 40);
  recorder.add(frame([red, red, white, white, blue, blue, white, white]), 0);
  recorder.add(frame([red, red, white, white, blue, blue, white, white]), 40); // identique : durée rallongée
  recorder.add(frame([blue, blue, white, white, red, red, white, white]), 120); // différente
  const gif = recorder.finish();

  it("écrit un GIF89a aux dimensions de l'écran", () => {
    expect(new TextDecoder().decode(gif.slice(0, 6))).toBe("GIF89a");
    expect((gif[6] as number) | ((gif[7] as number) << 8)).toBe(w);
    expect((gif[8] as number) | ((gif[9] as number) << 8)).toBe(h);
    expect(gif[gif.length - 1]).toBe(0x3b);
  });
  it("fusionne l'image répétée et compte les durées", () => {
    expect(delays(gif)).toEqual([12, 4]); // 120 ms (deux images fusionnées) puis 40 ms
  });
  it("garde un nombre d'images égal au nombre d'images distinctes", () => {
    expect(recorder.frames).toBe(3);
  });
});

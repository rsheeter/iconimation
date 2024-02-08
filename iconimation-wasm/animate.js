// From https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:opsz,wght,FILL,GRAD@24,400,0,0
let ttf_url = "https://fonts.gstatic.com/s/materialsymbolsoutlined/v161/kJF1BvYX7BgnkSrUwT8OhrdQw4oELdPIeeII9v6oDMzByHX9rA6RzaxHMPdY43zj-jCxv3fzvRNU22ZXGJpEpjC_1v-p_4MrImHCIJIZrDCvHOem.ttf";

var font_buffer;

import init, { generate_lottie } from './pkg/iconimation_wasm.js';
await init(); // Init Wasm

// Everyone returns promises so we end up with a lot of then's
fetch(ttf_url)
.then((response) => response.arrayBuffer())
.then((buffer) => {
    font_buffer = buffer;
    console.log("Font received!");            
});

export function generate_and_play_animation(command) {
    console.log("generate_and_play_animation " + command);
    return generate_lottie(font_buffer, command);
}
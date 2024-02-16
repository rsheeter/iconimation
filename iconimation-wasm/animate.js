// From https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:opsz,wght,FILL,GRAD@24,400,0,0
// modified to not specify any axis positions
let ttf_url = "https://fonts.gstatic.com/s/materialsymbolsoutlined/v161/kJEhBvYX7BgnkSrUwT8OhrdQw4oELdPIeeII9v6oFsc.ttf";

import init, { generate_lottie } from './pkg/iconimation_wasm.js';

var font_buffer = null;
let result = document.getElementById("result");
let player = document.getElementById("player")
let lottie_content = document.getElementById("lottie_content");

export async function initialize() {
    console.log("Initialize...");
    // Everyone returns promises, await all the things!
    await init(); // Init Wasm
    font_buffer = await (await fetch(ttf_url)).arrayBuffer();
    console.log("Font received!");
    result.innerText = `${font_buffer.byteLength} byte font ready for action!`
}

export function generate_and_play_animation(command) {
    command = command.trim();
    console.log("generate_and_play_animation " + command);
    result.innerText = "Generating... ";
    lottie_content.innerText = '';
    var lottie;
    try {
        lottie = generate_lottie(font_buffer, command);
        lottie = JSON.parse(lottie);
    } catch (e) {
        let message = `ERROR ${e}`;
        console.log(message);
        result.innerText += message;
        return;
    }
    result.innerText += "success!\n" + lottie.nm;
    console.log(lottie.nm);
    lottie_content.innerText = JSON.stringify(lottie, null, 2);
    console.log(lottie);
    player.load(lottie);
    player.play();
}
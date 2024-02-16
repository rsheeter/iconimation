// From https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:opsz,wght,FILL,GRAD@24,400,0,0
// modified to not specify any axis positions
let ttf_url = "https://fonts.gstatic.com/s/materialsymbolsoutlined/v161/kJEhBvYX7BgnkSrUwT8OhrdQw4oELdPIeeII9v6oFsc.ttf";

import init, { generate_animation } from './pkg/iconimation_wasm.js';

var font_buffer = null;
let result = document.getElementById("result");
let player = document.getElementById("player")
let lottie_content = document.getElementById("lottie_content");
let avd_content = document.getElementById("avd_content");

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
    var animation;
    try {
        animation = generate_animation(font_buffer, command);
        animation = JSON.parse(animation);
    } catch (e) {
        let message = `ERROR ${e}`;
        console.log(message);
        result.innerText += message;
        return;
    }
    result.innerText += "success!\n" + animation.debug;
    lottie_content.innerText = JSON.stringify(animation.lottie, null, 2);
    avd_content.innerText = JSON.stringify(animation.avd, null, 2);
    console.log(animation);
    player.load(animation.lottie);
    player.play();
}
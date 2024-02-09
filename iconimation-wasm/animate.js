// From https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:opsz,wght,FILL,GRAD@24,400,0,0
let ttf_url = "https://fonts.gstatic.com/s/materialsymbolsoutlined/v161/kJF1BvYX7BgnkSrUwT8OhrdQw4oELdPIeeII9v6oDMzByHX9rA6RzaxHMPdY43zj-jCxv3fzvRNU22ZXGJpEpjC_1v-p_4MrImHCIJIZrDCvHOem.ttf";

import init, { generate_lottie } from './pkg/iconimation_wasm.js';

var font_buffer = null;

export async function initialize() {
    console.log("Initialize...");
    // Everyone returns promises, await all the things!
    await init(); // Init Wasm
    font_buffer = await (await fetch(ttf_url)).arrayBuffer();
    console.log("Font received!");
}

let result = document.getElementById("result");
let player = document.getElementById("player")
let lottie_content = document.getElementById("lottie_content");

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
    result.innerText += "success!";
    console.log(lottie);
    lottie_content.innerText = JSON.stringify(lottie, null, 2);
    console.log(lottie);
    player.load(lottie);
    player.play();
}
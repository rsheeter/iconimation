# iconimation
Exploratory hacking around generating animation from a Google-style variable icon font.

Given a font and a description of the desired animation, generates an abstract representation
of the animation and then emits multiple output formats to support multiple target platforms.

Don't count on anything working correctly.

## Try it

### CLI

1. Find a Google-style icon font
   * `git clone git@github.com:google/material-design-icons.git` perhaps
   * browse at https://fonts.google.com/icons
1. Generate an animation

    ```shell
    # Example assumes that:
    # 1) We are in the root of this repo
    # 2) git@github.com:google/material-design-icons.git is cloned sibling to current directory
   
   # See iconimation-wasm/wasm-demo.html in this repo for more sample commands
    $ cargo run -- -c "Animate more_horiz: pulse" -f ../material-design-icons/variablefont/MaterialSymbolsOutlined\[FILL\,GRAD\,opsz\,wght\].ttf
    ```

1. Try it out
   * https://lottiefiles.github.io/lottie-docs/playground/json_editor/ perhaps?
   * To generate a lottie and place it on the copy buffer so you can paste it into ^
   `$ cargo run -- -c "Animate more_horiz: pulse" -f ../material-design-icons/variablefont/MaterialSymbolsOutlined\[FILL\,GRAD\,opsz\,wght\].ttf && cat lottie.json | xclip -selection c`

### Wasm

```shell
# Once
$ cargo install wasm-pack

# Each time
$ wasm-pack build iconimation-wasm --target web
$ (cd iconimation-wasm && python -m http.server 8010)
# load http://localhost:8010/demo.html
```
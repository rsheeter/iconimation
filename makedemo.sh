#!/usr/bin/env bash

set -e

function generate() {
    local sample_file="$1"
    local animation="$2"
    local template="$3"
    local more_flags="$4"
    local font="$5"
    # strip comments
    sed 's/#.*//g' "$sample_file" \
    | grep -v "^\S*$" \
    | awk "{ print \"--codepoint 0x\"\$2\" --animation $animation --template resources/templates/"$template" $more_flags --out-file demo/lottie/\"\$1\"-$animation-$template\" } " \
    | xargs -L1 target/release/iconimation-cli --font "$font" --debug
}

rm -f demo/all.json
mkdir -p demo/lottie

font='../material-design-icons/variablefont/MaterialSymbolsOutlined[FILL,GRAD,opsz,wght].ttf'
sample_file=samples2.txt
cargo build --release

#generate samples2.txt pulse-whole "$font"
#generate samples2.txt pulse-parts "$font"

generate samples.txt none ScalePosition.json "" "$font"
generate samples.txt none ScaleRotate.json "" "$font"
generate samples.txt none ScaleRotatePosition.json "" "$font"

python3 makedemo.py
cp demo.html demo/

echo "Try demo/demo.html"
# Utility to dump the animation parameters of a template
#
# Assumes template has a group named "placeholder" with an animated transform.
#
# Usage:
#    python3 dump_motion.py resources/templates/ScaleRotate.json
#
#

import json
import matplotlib.pyplot as plt
import numpy as np
from pathlib import Path
import sys


def find(json, attr, value):
    frontier = [json]
    while frontier:
        curr = frontier.pop(0)
        if type(curr) == dict:
            for k,v in curr.items():
                if k == attr and v == value:
                    return curr
                elif type(v) == dict:
                    frontier.append(v)
                elif type(v) == list:
                    frontier.append(v)
        elif type(curr) == list:
            frontier.extend(curr)
        else:
            print("wut", type(curr))
    return None

def plot_keyframes(keyframes, key, name, series_names):
    # https://matplotlib.org/stable/plot_types/basic/scatter_plot.html
    plt.style.use('_mpl-gallery')
    keyframes.sort(key=lambda k: k["t"])  # time in frames

    # borrowed from https://matplotlib.org/stable/api/_as_gen/matplotlib.axes.Axes.scatter.html#matplotlib.axes.Axes.scatter
    series_colors = ['#1f77b4', '#ff7f0e', '#2ca02c', '#d62728', '#9467bd', '#8c564b', '#e377c2', '#7f7f7f', '#bcbd22', '#17becf']

    x = []
    y = []
    colors = []

    for keyframe in keyframes:
        time = keyframe["t"]
        values = keyframe["s"]
        if len(values) != len(series_names):
            raise ValueError(f"{series_names} wrong length for {values}")
        for (i, value) in enumerate(values):
            x.append(time)
            y.append(value)
            colors.append(series_colors[i])

    fig, ax = plt.subplots()
    ax.scatter(x, y, c=colors)
    fig.suptitle(name)

    return fig


def main(argv):
    with open("motion.html", "w") as f_out:
        print("<!DOCTYPE html>", file=f_out)
        print("<html>", file=f_out)
        print("<body>", file=f_out)

        for lottie_file in argv[1:]:
            lottie_file = Path(lottie_file)
            if not lottie_file.suffix == ".json":
                print(f"{lottie_file} doesn't look like a Lottie?")
                continue
            with open(lottie_file) as f:
                lottie = json.load(f)

            placeholder = find(lottie, "nm", "placeholder")
            if placeholder is None:
                print(f"{lottie_file} has no placeholder :(")
                continue
            shapes = placeholder.get("it", [{}])
            transform = shapes[-1]
            if transform.get("ty", "?") != "tr":
                print("The last item is not a transform!")
                continue

            print(f"<h3>{lottie_file.name}</h3>", file=f_out)

            # this is not every possible animated field of a transform, just the basics
            # Ref https://lottiefiles.github.io/lottie-docs/concepts/#animated-property
            svg_files = []
            for (key, name, series_names) in (("p", "position", ("x", "y")), ("s", "scale", ("sx", "sy")), ("r", "rotation", ("",))):
                maybe_animated = transform.get(key, None)
                if maybe_animated is None:
                    print(f"No {name} at all?!")
                    continue
                is_animated = maybe_animated.get("a", 0)
                if not is_animated:
                    print(f"{name} is not animated")
                    continue
                keyframes = maybe_animated.get("k", [])
                if not keyframes:
                    print(f"{name} IS animated but has no keyframes. Very suspicious!")
                    continue

                plot = plot_keyframes(keyframes, key, name, series_names)
                svg_file = lottie_file.stem + "." + name + ".svg"
                plot.savefig(svg_file)
                print(f"{name} is animated, {len(keyframes)} keyframes dumped to {svg_file}")
                svg_files.append(svg_file)

            for svg_file in svg_files:
                with open(svg_file) as svg_f:
                    svg = svg_f.read()
                print(svg, file=f_out)

        print("</body>", file=f_out)
        print("</html>", file=f_out)


if __name__ == "__main__":
    main(sys.argv)
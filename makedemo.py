
from pathlib import Path



jsons = Path("demo/lottie").glob("*.json")

with open("demo/all.json", "w") as f:
    arr = ",\n".join(sorted([f'"{j.name}"' for j in jsons]))
    print(f'[{arr}]', file=f)

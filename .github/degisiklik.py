import re, subprocess, sys

onceki_yol, cikti_yol = sys.argv[3], sys.argv[4]
prev, run = sys.argv[1], sys.argv[2]
ILK_SURUM = 13  # bu sürümden öncesi listeye alınmaz


def git(*a):
    return subprocess.run(["git", *a], capture_output=True, text=True, encoding="utf-8").stdout


aralik = [f"{prev}..HEAD"] if prev else ["-1"]
ham = git("log", "--reverse", "--format=%x1e%s%x1f%b", *aralik)
baslik, maddeler = "", []
for kayit in ham.split("\x1e"):
    if not kayit.strip():
        continue
    konu, _, govde = kayit.partition("\x1f")
    konu = konu.strip()
    m = [l.strip()[2:].strip() for l in govde.splitlines() if l.strip().startswith("- ")]
    baslik = konu or baslik
    for x in (m or ([konu] if konu else [])):
        if x not in maddeler:
            maddeler.append(x)

yeni = f"## 0.4.{run} — {baslik or 'Güncelleme'}\n" + "".join(f"- {x}\n" for x in maddeler)

try:
    eski = open(onceki_yol, encoding="utf-8").read()
except OSError:
    eski = ""
parcalar = re.split(r"(?m)^(?=## )", eski)
tut = []
for p in parcalar:
    mm = re.match(r"## 0\.4\.(\d+)", p)
    if mm and int(mm.group(1)) >= ILK_SURUM and f"## 0.4.{run} " not in p:
        tut.append(p if p.endswith("\n") else p + "\n")
open(cikti_yol, "w", encoding="utf-8").write(yeni + "".join(tut))

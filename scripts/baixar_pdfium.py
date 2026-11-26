#!/usr/bin/env python3
"""Baixa a biblioteca PDFium para vendor/pdfium/ (so biblioteca padrao do Python).

Uso:
    python3 scripts/baixar_pdfium.py                 # plataforma desta maquina
    python3 scripts/baixar_pdfium.py --plataforma win-x64
    python3 scripts/baixar_pdfium.py --destino <pasta>   # padrao: vendor/pdfium

A versao e o SHA-256 de cada pacote ficam fixos aqui: o arquivo baixado so e
usado se o hash bater. Ao trocar a versao, troque tudo junto (o `chromium/NNNN`
tem de ser o da feature `pdfium_latest` do pdfium-render; veja docs/pdfium.md).

Resultado, igual em todas as plataformas:
    <destino>/lib/<biblioteca>   (libpdfium.dylib, pdfium.dll ou libpdfium.so)
    <destino>/LICENSE
"""

import argparse
import hashlib
import io
import platform
import shutil
import sys
import tarfile
import urllib.request
from pathlib import Path

RELEASE = "chromium/6996"
URL_BASE = "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F6996"

# plataforma -> (sha256 do .tgz, caminho da biblioteca dentro do .tgz)
PACOTES = {
    "mac-arm64": ("1b103d5ebfd8f7b5720ca9ba76888e7efaa80678a49c8f3da3b0ac64ba5c90fb", "lib/libpdfium.dylib"),
    "mac-x64": ("66162d9aa0b059fbd9e54a47a62cbd582cc6ab8caee0829f2252d9bcd4750e88", "lib/libpdfium.dylib"),
    "win-x64": ("83e714c302ceacccf403826d5cb57ea39b77f393d83b8d5781283012774a9378", "bin/pdfium.dll"),
    "linux-x64": ("68b381b87efed539f2e33ae1e280304c9a42643a878cc296c1d66a93b0cb4335", "lib/libpdfium.so"),
}


def plataforma_atual() -> str:
    so = {"Darwin": "mac", "Windows": "win", "Linux": "linux"}.get(platform.system())
    arq = {"arm64": "arm64", "aarch64": "arm64", "x86_64": "x64", "amd64": "x64"}.get(platform.machine().lower())
    nome = f"{so}-{arq}" if so and arq else None
    if nome not in PACOTES:
        sys.exit(f"sem pacote para {platform.system()}/{platform.machine()}; use --plataforma ({', '.join(PACOTES)})")
    return nome


def baixar(plataforma: str, destino: Path) -> Path:
    sha_esperado, caminho_lib = PACOTES[plataforma]
    url = f"{URL_BASE}/pdfium-{plataforma}.tgz"
    print(f"baixando {url}")
    with urllib.request.urlopen(url) as r:
        dados = r.read()
    sha = hashlib.sha256(dados).hexdigest()
    if sha != sha_esperado:
        sys.exit(f"SHA-256 diferente do esperado\n  esperado: {sha_esperado}\n  obtido:   {sha}")
    lib_dir = destino / "lib"
    lib_dir.mkdir(parents=True, exist_ok=True)
    with tarfile.open(fileobj=io.BytesIO(dados), mode="r:gz") as tar:
        for membro, saida in ((caminho_lib, lib_dir / Path(caminho_lib).name), ("LICENSE", destino / "LICENSE")):
            f = tar.extractfile(membro)  # le so os dois membros conhecidos: nada de extractall
            if f is None:
                sys.exit(f"{membro} nao esta no pacote")
            with open(saida, "wb") as out:
                shutil.copyfileobj(f, out)
    return lib_dir / Path(caminho_lib).name


def main() -> None:
    raiz = Path(__file__).resolve().parent.parent
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--plataforma", choices=sorted(PACOTES), default=None)
    ap.add_argument("--destino", type=Path, default=raiz / "vendor" / "pdfium")
    a = ap.parse_args()
    lib = baixar(a.plataforma or plataforma_atual(), a.destino)
    print(f"ok ({RELEASE}): {lib}")


if __name__ == "__main__":
    main()

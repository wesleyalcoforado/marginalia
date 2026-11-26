#!/usr/bin/env python3
"""Gera crates/ui/assets/app/marginalia.ico a partir de marginalia.png (so biblioteca padrao + `sips` do macOS).

O .ico leva varios tamanhos, cada um como PNG (aceito desde o Windows Vista). Rode no macOS quando o PNG mudar e
versione o .ico: o build do Windows (crates/ui/build.rs) o embute no executavel.
"""
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
PASTA = RAIZ / "crates" / "ui" / "assets" / "app"
TAMANHOS = [16, 24, 32, 48, 64, 128, 256]


def main() -> None:
    origem = PASTA / "marginalia.png"
    imagens = []
    with tempfile.TemporaryDirectory() as tmp:
        for t in TAMANHOS:
            saida = Path(tmp) / f"{t}.png"
            subprocess.run(["sips", "-z", str(t), str(t), str(origem), "--out", str(saida)],
                           check=True, capture_output=True)
            imagens.append((t, saida.read_bytes()))
    cabecalho = struct.pack("<HHH", 0, 1, len(imagens))
    deslocamento = 6 + 16 * len(imagens)
    entradas, dados = b"", b""
    for t, png in imagens:
        lado = 0 if t >= 256 else t  # 0 significa 256
        entradas += struct.pack("<BBBBHHII", lado, lado, 0, 0, 1, 32, len(png), deslocamento + len(dados))
        dados += png
    destino = PASTA / "marginalia.ico"
    destino.write_bytes(cabecalho + entradas + dados)
    print(f"gerado {destino} ({destino.stat().st_size} bytes)", file=sys.stderr)


if __name__ == "__main__":
    main()

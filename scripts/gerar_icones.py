#!/usr/bin/env python3
"""Gera os icones derivados de crates/ui/assets/app/marginalia.png (mestre 1024x1024, quadrado arredondado com margem
transparente). Precisa do ImageMagick (`magick`); rode quando o mestre mudar e versione os arquivos gerados.

- marginalia-macos.png: a mesma arte sem a margem transparente e sem o aro claro da borda, preenchendo o quadrado todo
  (opaco). O macOS recorta o icone com a propria mascara; um icone com margem transparente aparece dentro de uma
  placa branca. Os cantos e as faixas que a ampliacao deixa vazias sao preenchidos com o degrade do fundo.
- marginalia-<N>.png: tamanhos para a tela de abertura e a janela Sobre. O gpui reduz textura sem filtro de qualidade
  (1024 -> 96 px fica craquelado no Windows), entao a interface escolhe o tamanho mais proximo do fisico
  (`icons::app_icon`) e a imagem sai 1:1, sem reduzir na GPU.
- marginalia.ico: varios tamanhos em PNG, embutido no .exe pelo build.rs.
"""
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

PASTA = Path(__file__).resolve().parent.parent / "crates" / "ui" / "assets" / "app"
MESTRE = PASTA / "marginalia.png"
TAMANHOS_UI = [96, 120, 144, 192, 288]  # 96 px logicos nas escalas 1, 1.25, 1.5, 2 e 3 (mantenha igual em icons.rs)
TAMANHOS_ICO = [16, 24, 32, 48, 64, 128, 256]


def magick(*args) -> None:
    subprocess.run(["magick", *map(str, args)], check=True)


def redimensionar(destino: Path, lado: int) -> None:
    magick(MESTRE, "-filter", "Lanczos", "-resize", f"{lado}x{lado}", destino)


def sem_margem(destino: Path) -> None:
    with tempfile.TemporaryDirectory() as tmp:
        t = Path(tmp)
        # mascara: o quadrado arredondado erodido (tira o aro claro) e suavizado, ampliado 124% sobre 1024
        magick(MESTRE, "-alpha", "extract", "-morphology", "Erode", "Disk:40", "-blur", "0x26", "-resize", "124%",
               "-gravity", "center", "-extent", "1024x1024", t / "mascara.png")
        magick(MESTRE, "-alpha", "off", "-resize", "124%", "-gravity", "center", "-extent", "1024x1024", t / "arte.png")
        # degrade do fundo (amostrado na lateral do quadrado, extrapolado linearmente)
        magick("-size", "1024x1024", "xc:", "-sparse-color", "Barycentric",
               "50.7,187.1 srgb(21,91,175) 50.7,807.1 srgb(1,26,67) 968.3,187.1 srgb(3,46,99)", t / "fundo.png")
        magick(t / "fundo.png", "(", t / "arte.png", t / "mascara.png", "-alpha", "off", "-compose", "CopyOpacity",
               "-composite", ")", "-compose", "Over", "-composite", "-alpha", "off", destino)


def ico(destino: Path) -> None:
    imagens = []
    with tempfile.TemporaryDirectory() as tmp:
        for t in TAMANHOS_ICO:
            saida = Path(tmp) / f"{t}.png"
            redimensionar(saida, t)
            imagens.append((t, saida.read_bytes()))
    cabecalho = struct.pack("<HHH", 0, 1, len(imagens))
    deslocamento = 6 + 16 * len(imagens)
    entradas, dados = b"", b""
    for t, png in imagens:
        lado = 0 if t >= 256 else t  # 0 significa 256
        entradas += struct.pack("<BBBBHHII", lado, lado, 0, 0, 1, 32, len(png), deslocamento + len(dados))
        dados += png
    destino.write_bytes(cabecalho + entradas + dados)


def main() -> None:
    sem_margem(PASTA / "marginalia-macos.png")
    for lado in TAMANHOS_UI:
        redimensionar(PASTA / f"marginalia-{lado}.png", lado)
    ico(PASTA / "marginalia.ico")
    print("icones gerados em", PASTA, file=sys.stderr)


if __name__ == "__main__":
    main()

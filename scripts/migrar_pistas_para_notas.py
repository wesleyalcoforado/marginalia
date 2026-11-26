#!/usr/bin/env python3
"""Migra um vault do Marginalia da versao com "pistas" para a versao com "notas".

O que muda no vault (nada alem disto):
  - a pasta `pistas/` vira `notas/`;
  - nas tarefas (`tarefas/*.md`), a chave de frontmatter `pista:` vira `nota:`.
O id dos documentos nao muda, entao tudo que os cita continua valendo. Os grifos, as fontes e
os registros nao mudam.

Uso:
    python3 scripts/migrar_pistas_para_notas.py <pasta-do-vault>             # migra
    python3 scripts/migrar_pistas_para_notas.py <pasta-do-vault> --simular   # so mostra o que faria

Recusa o vault que ja tem `notas/` ao lado de `pistas/` (nao sabe qual das duas vale); nesse
caso junte as duas a mao. Pode ser rodado de novo (idempotente) e, se parar no meio, termina
na proxima vez: as tarefas sao regravadas primeiro e a pasta e renomeada por ultimo.
Faca uma copia do vault antes. So biblioteca padrao: o frontmatter e tratado por linhas.
"""
import argparse
import os
import re
import sys

CHAVE_ANTIGA = re.compile(r"^pista:(.*)$")
CHAVE_NOVA = re.compile(r"^nota:")


def migrar_texto(texto):
    """Troca `pista:` por `nota:` no frontmatter (so no topo, so ate o segundo `---`).

    Devolve (texto novo, avisos). Sem frontmatter, ou sem `pista:`, devolve o texto como esta.
    Se o documento ja tem `nota:` e `pista:` ao mesmo tempo, nao mexe e avisa.
    """
    linhas = texto.split("\n")
    if not linhas or linhas[0].rstrip() != "---":
        return texto, []
    fim = None
    for i in range(1, len(linhas)):
        if linhas[i].rstrip() == "---":
            fim = i
            break
    if fim is None:
        return texto, []
    cab = linhas[1:fim]
    antigas = [i for i, l in enumerate(cab) if CHAVE_ANTIGA.match(l)]
    if not antigas:
        return texto, []
    if any(CHAVE_NOVA.match(l) for l in cab):
        return texto, ["tem `pista:` e `nota:` ao mesmo tempo"]
    for i in antigas:
        cab[i] = "nota:" + CHAVE_ANTIGA.match(cab[i]).group(1)
    return "\n".join(linhas[:1] + cab + linhas[fim:]), []


def ler(caminho):
    # newline="" preserva \r\n de arquivos editados no Windows
    with open(caminho, encoding="utf-8", newline="") as f:
        return f.read()


def gravar(caminho, texto):
    tmp = caminho + ".tmp"
    with open(tmp, "w", encoding="utf-8", newline="") as f:
        f.write(texto)
    os.replace(tmp, caminho)


def migrar(raiz, simular=False, saida=print):
    """Migra o vault em `raiz`. Devolve o codigo de saida (0 ok, 1 erro)."""
    pistas = os.path.join(raiz, "pistas")
    notas = os.path.join(raiz, "notas")
    tarefas = os.path.join(raiz, "tarefas")
    if not os.path.isdir(raiz):
        saida(f"erro: {raiz} nao e uma pasta")
        return 1
    if os.path.isdir(pistas) and os.path.isdir(notas):
        saida(f"erro: {raiz} tem `pistas/` e `notas/` ao mesmo tempo; junte as duas a mao e rode de novo")
        return 1
    if not os.path.isdir(pistas):
        if os.path.isdir(notas):
            saida("nada a fazer: o vault ja usa `notas/`")
            return 0
        saida(f"erro: {raiz} nao parece um vault do Marginalia (sem `pistas/` nem `notas/`)")
        return 1

    mudadas, avisos = 0, 0
    if os.path.isdir(tarefas):
        for nome in sorted(os.listdir(tarefas)):
            if not nome.endswith(".md"):
                continue
            caminho = os.path.join(tarefas, nome)
            novo, av = migrar_texto(ler(caminho))
            for a in av:
                saida(f"aviso: tarefas/{nome}: {a}; deixada como esta")
                avisos += 1
            if novo != ler(caminho):
                mudadas += 1
                saida(f"tarefas/{nome}: `pista:` -> `nota:`")
                if not simular:
                    gravar(caminho, novo)
    saida(f"pistas/ -> notas/")
    if not simular:
        os.rename(pistas, notas)
    saida(f"{'(simulacao) ' if simular else ''}{mudadas} tarefa(s) atualizada(s), {avisos} aviso(s)")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("vault", help="pasta do vault")
    ap.add_argument("--simular", action="store_true", help="so mostra o que faria")
    a = ap.parse_args()
    sys.exit(migrar(a.vault, a.simular))


if __name__ == "__main__":
    main()

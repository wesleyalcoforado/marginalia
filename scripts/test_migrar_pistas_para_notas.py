"""Testes do script de migracao pistas -> notas (so biblioteca padrao)."""
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import migrar_pistas_para_notas as m  # noqa: E402

IMAGEM = "imagem:\n  fonte: Livro\n  arquivo: a.jpg\n  pagina: 1\n"


def tarefa(id, extra="", corpo="achar o testamento\n"):
    return f"---\nid: {id}\nestado: aberta\n{IMAGEM}{extra}---\n{corpo}"


class Vault:
    def __init__(self, pasta_notas="pistas"):
        self.tmp = tempfile.TemporaryDirectory()
        self.raiz = self.tmp.name
        for d in ("fontes", pasta_notas, "tarefas", "grifos"):
            os.makedirs(os.path.join(self.raiz, d))

    def escrever(self, pasta, nome, texto):
        with open(os.path.join(self.raiz, pasta, nome), "w", encoding="utf-8", newline="") as f:
            f.write(texto)

    def ler(self, pasta, nome):
        with open(os.path.join(self.raiz, pasta, nome), encoding="utf-8", newline="") as f:
            return f.read()

    def existe(self, *partes):
        return os.path.exists(os.path.join(self.raiz, *partes))


def rodar(v, **kw):
    linhas = []
    codigo = m.migrar(v.raiz, saida=linhas.append, **kw)
    return codigo, linhas


class MigrarTexto(unittest.TestCase):
    def test_troca_a_chave_no_frontmatter(self):
        novo, av = m.migrar_texto(tarefa("t1", "pista: p1\n"))
        self.assertEqual(novo, tarefa("t1", "nota: p1\n"))
        self.assertEqual(av, [])

    def test_so_a_chave_de_topo_do_frontmatter(self):
        corpo = "pista: isto e texto, nao frontmatter\n"
        original = tarefa("t1", "extra:\n  pista: aninhada\n", corpo)
        novo, _ = m.migrar_texto(original)
        self.assertEqual(novo, original, "indentada (aninhada) e corpo ficam como estao")

    def test_sem_a_chave_ou_sem_frontmatter_nao_muda(self):
        for t in (tarefa("t1"), "texto solto\npista: x\n", "---\nsem fim\npista: x\n"):
            self.assertEqual(m.migrar_texto(t), (t, []))

    def test_nota_e_pista_juntas_avisa_e_nao_mexe(self):
        t = tarefa("t1", "pista: a\nnota: b\n")
        novo, av = m.migrar_texto(t)
        self.assertEqual(novo, t)
        self.assertEqual(len(av), 1)

    def test_preserva_fim_de_linha_do_windows(self):
        t = "---\r\nid: t1\r\npista: p1\r\n---\r\ncorpo\r\n"
        novo, _ = m.migrar_texto(t)
        self.assertEqual(novo, "---\r\nid: t1\r\nnota: p1\r\n---\r\ncorpo\r\n")


class MigrarVault(unittest.TestCase):
    def test_renomeia_a_pasta_e_troca_a_chave_das_tarefas(self):
        v = Vault()
        v.escrever("pistas", "p1.md", "---\nid: p1\n---\numa nota\n")
        v.escrever("tarefas", "t1.md", tarefa("t1", "pista: p1\n"))
        v.escrever("tarefas", "t2.md", tarefa("t2"))
        codigo, _ = rodar(v)
        self.assertEqual(codigo, 0)
        self.assertFalse(v.existe("pistas"))
        self.assertEqual(v.ler("notas", "p1.md"), "---\nid: p1\n---\numa nota\n", "documento intacto")
        self.assertEqual(v.ler("tarefas", "t1.md"), tarefa("t1", "nota: p1\n"))
        self.assertEqual(v.ler("tarefas", "t2.md"), tarefa("t2"))

    def test_simular_nao_muda_nada(self):
        v = Vault()
        v.escrever("tarefas", "t1.md", tarefa("t1", "pista: p1\n"))
        codigo, linhas = rodar(v, simular=True)
        self.assertEqual(codigo, 0)
        self.assertTrue(v.existe("pistas") and not v.existe("notas"))
        self.assertEqual(v.ler("tarefas", "t1.md"), tarefa("t1", "pista: p1\n"))
        self.assertTrue(any("t1.md" in l for l in linhas))

    def test_rodar_de_novo_nao_faz_nada(self):
        v = Vault()
        v.escrever("tarefas", "t1.md", tarefa("t1", "pista: p1\n"))
        rodar(v)
        antes = v.ler("tarefas", "t1.md")
        codigo, linhas = rodar(v)
        self.assertEqual(codigo, 0)
        self.assertEqual(v.ler("tarefas", "t1.md"), antes)
        self.assertTrue(any("nada a fazer" in l for l in linhas))

    def test_termina_uma_migracao_interrompida_antes_de_renomear_a_pasta(self):
        v = Vault()
        v.escrever("tarefas", "t1.md", tarefa("t1", "nota: p1\n"))  # ja regravada
        v.escrever("tarefas", "t2.md", tarefa("t2", "pista: p2\n"))  # faltava
        codigo, _ = rodar(v)
        self.assertEqual(codigo, 0)
        self.assertTrue(v.existe("notas") and not v.existe("pistas"))
        self.assertEqual(v.ler("tarefas", "t2.md"), tarefa("t2", "nota: p2\n"))

    def test_recusa_com_pistas_e_notas_ao_mesmo_tempo(self):
        v = Vault()
        os.makedirs(os.path.join(v.raiz, "notas"))
        v.escrever("tarefas", "t1.md", tarefa("t1", "pista: p1\n"))
        codigo, _ = rodar(v)
        self.assertEqual(codigo, 1)
        self.assertTrue(v.existe("pistas") and v.existe("notas"))
        self.assertEqual(v.ler("tarefas", "t1.md"), tarefa("t1", "pista: p1\n"))

    def test_recusa_pasta_que_nao_e_vault(self):
        with tempfile.TemporaryDirectory() as d:
            self.assertEqual(m.migrar(d, saida=lambda _: None), 1)
        self.assertEqual(m.migrar("/nao/existe/mesmo", saida=lambda _: None), 1)

    def test_tarefa_com_as_duas_chaves_avisa_e_o_resto_migra(self):
        v = Vault()
        v.escrever("tarefas", "t1.md", tarefa("t1", "pista: a\nnota: b\n"))
        v.escrever("tarefas", "t2.md", tarefa("t2", "pista: c\n"))
        codigo, linhas = rodar(v)
        self.assertEqual(codigo, 0)
        self.assertEqual(v.ler("tarefas", "t1.md"), tarefa("t1", "pista: a\nnota: b\n"))
        self.assertEqual(v.ler("tarefas", "t2.md"), tarefa("t2", "nota: c\n"))
        self.assertTrue(any(l.startswith("aviso") and "t1.md" in l for l in linhas))


if __name__ == "__main__":
    unittest.main()

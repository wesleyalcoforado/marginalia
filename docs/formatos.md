# Formatos e limitações

Decodificação por página em `nucleo::decode::decode_page` (formato pela extensão, sem distinguir maiúsculas): jpg, jpeg, png, tif, tiff, pdf.

## TIFF

Suportado:
- cinza, cinza+alfa, RGB e RGBA de 8 bits e de 16 bits (16 bits é reduzido com `>> 8`);
- 1 bit (bilevel, cinza) com compressão nenhuma, LZW, PackBits ou Deflate, strips (não tiles), WhiteIsZero e BlackIsZero.
  O decodificador de 1 bit é próprio (o `tiff` 0.9.1 não desempacota linhas de 1 bit).

Não suportado (devolve `DecodeError::NaoSuportado` com mensagem clara, nunca pixels errados):
- TIFF com paleta (PhotometricInterpretation=3 / ColorMap);
- CCITT Group 3 e Group 4 (Fax3/Fax4), muito comuns em scans em preto e branco;
- TIFF de 1 bit com FillOrder=2 (bits invertidos) ou com Predictor diferente de 1 (seriam pixels errados);
- imagens TIFF acima de `MAX_PIXELS` = 200 megapixels (largura x altura), recusadas antes de qualquer alocacao;
- outras profundidades de bits (2, 4, 32 etc.), CMYK, YCbCr, 1 bit em tiles.

Esses limites vêm do crate `tiff`. Próximo passo sugerido: usar o crate `fax` para decodificar G4/G3 (e converter paleta manualmente).

## PDF

Renderizado com PDFium (ver `pdfium.md`), maior lado limitado a 2400 px, fundo branco.

Nota: strips de 1 bit que descomprimem além do tamanho declarado, ou cujos offsets/contagens apontam além do arquivo, são `Invalido`.

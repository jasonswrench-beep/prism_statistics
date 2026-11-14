# Writes two_sheets.xlsx: a minimal workbook built from plain XML, so it carries no program metadata.
# Sheet "Survey": id, group, score (one empty), visit (dates; one empty). Sheet "Codebook": a note.
# Run from this folder: python make_two_sheets.py
import zipfile

def cell(ref, v):
    if v is None:
        return ""
    if isinstance(v, str):
        return f'<c r="{ref}" t="inlineStr"><is><t>{v}</t></is></c>'
    if isinstance(v, tuple):  # ("date", serial)
        return f'<c r="{ref}" s="1"><v>{v[1]}</v></c>'
    return f'<c r="{ref}"><v>{v}</v></c>'

def sheet(rows):
    body = ""
    for r, row in enumerate(rows, start=1):
        body += f'<row r="{r}">' + "".join(cell(f"{chr(65 + c)}{r}", v) for c, v in enumerate(row)) + "</row>"
    return f'<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>{body}</sheetData></worksheet>'

survey = [["id", "group", "score", "visit"], [1, 1, 4.5, ("date", 46027)], [2, 2, 3, ("date", 46028)], [3, 1, None, ("date", 46029)], [4, 2, 5.5, None]]
codebook = [["variable", "meaning"], ["group", "1 = control, 2 = treatment"]]
ct = "application/vnd.openxmlformats-officedocument.spreadsheetml"
with zipfile.ZipFile("two_sheets.xlsx", "w", zipfile.ZIP_DEFLATED) as z:
    z.writestr("[Content_Types].xml", f'<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="{ct}.sheet.main+xml"/><Override PartName="/xl/styles.xml" ContentType="{ct}.styles+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="{ct}.worksheet+xml"/><Override PartName="/xl/worksheets/sheet2.xml" ContentType="{ct}.worksheet+xml"/></Types>')
    z.writestr("_rels/.rels", '<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>')
    z.writestr("xl/workbook.xml", '<?xml version="1.0"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Survey" sheetId="1" r:id="rId1"/><sheet name="Codebook" sheetId="2" r:id="rId2"/></sheets></workbook>')
    z.writestr("xl/_rels/workbook.xml.rels", '<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/></Relationships>')
    z.writestr("xl/styles.xml", '<?xml version="1.0"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="1"><font><sz val="11"/></font></fonts><fills count="1"><fill><patternFill patternType="none"/></fill></fills><borders count="1"><border/></borders><cellStyleXfs count="1"><xf/></cellStyleXfs><cellXfs count="2"><xf numFmtId="0"/><xf numFmtId="14" applyNumberFormat="1"/></cellXfs></styleSheet>')
    z.writestr("xl/worksheets/sheet1.xml", sheet(survey))
    z.writestr("xl/worksheets/sheet2.xml", sheet(codebook))
print("wrote two_sheets.xlsx")

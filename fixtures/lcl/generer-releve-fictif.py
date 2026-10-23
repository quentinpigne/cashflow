"""Génère `releve-fictif.pdf` : un relevé de compte au format LCL, entièrement inventé (invariant 7).

Il reproduit la mise en page d'un vrai relevé (en-têtes, colonnes, suites de libellé, soldes, totaux,
pied de page) pour tester l'extraction PDF de bout en bout (IMP-10). Aucune donnée réelle.

    python3 -m pip install reportlab && python3 fixtures/lcl/generer-releve-fictif.py
"""

from pathlib import Path

from reportlab.lib.pagesizes import portrait
from reportlab.pdfgen import canvas

OUT = Path(__file__).with_name("releve-fictif.pdf")
DEBIT_RIGHT, CREDIT_RIGHT = 460, 532

# (date, libellé et suites, valeur, débit, crédit)
PAGE_2 = [
    ("04.12", ["VIREMENT EMPLOYEUR FICTIF", "LIBELLE:SALAIRE NOVEMBRE", "REF.CLIENT:000000000"], "04.12.25", None, "2 400,00"),
    ("05.12", ["CB BOULANGERIE DU COIN 03/12/25"], "05.12.25", "4,20", None),
    ("06.12", ["CB Carte Ticket Res 04/12/25", "Paris      EUR         3,72"], "06.12.25", "12,50", None),
    ("08.12", ["PRLV SEPA ENERGIE FICTIVE", "LIBELLE:Mandat ++X000000", "ID.CREANCIER:FR00ZZZ000000"], "08.12.25", "1 016,00", None),
    ("15.12", ["CB SUPERMARCHE FICTIF 13/12/25"], "15.12.25", "87,35", None),
]
PAGE_3 = [
    ("31.12", ["REMISE FIDELITE TENUE DE COMPTE"], "31.12.25", None, "10,00"),
    ("02.01", ["CB LIBRAIRIE FICTIVE 31/12/25"], "02.01.26", "20,00", None),
]


def header(c, page, pages):
    c.setFont("Helvetica-Bold", 12)
    c.drawString(131.8, 797.8, "RELEVE DE COMPTE")
    c.setFont("Helvetica", 9)
    c.drawString(346.3, 766.6, "du 03.12.2025 au 02.01.2026 -  N° 12")
    c.drawString(38.9, 742.8, "M DUPONT JEAN")
    c.drawString(519.6, 37.0, f"Page {page} / {pages}")


def table(c, rows, y):
    c.setFont("Helvetica-Bold", 9)
    c.drawString(44.9, 694.9, "ECRITURES DE LA PERIODE")
    for x, name in [(41.6, "DATE"), (203.0, "LIBELLE"), (370.0, "VALEUR"), (438.0, "DEBIT"), (510.0, "CREDIT")]:
        c.drawString(x, 671.0, name)
    c.setFont("Helvetica", 9)
    for date, label, value, debit, credit in rows:
        c.drawString(42.5, y, date)
        c.drawString(74.8, y, label[0])
        c.drawString(365.6, y, value)
        if debit:
            c.drawRightString(DEBIT_RIGHT, y, debit)
        if credit:
            c.drawRightString(CREDIT_RIGHT, y, credit)
        y -= 11.95
        for more in label[1:]:
            c.drawString(80.8, y, more)
            y -= 11.95
    return y


c = canvas.Canvas(str(OUT), pagesize=portrait((612, 864)))
c.setAuthor("Fixture Cashflow")
header(c, 1, 3)
c.drawString(38.9, 730.8, "BANQUE FICTIVE")
c.drawString(44.9, 563.5, "IBAN : FR00 0000 0000 0000 0000 0000 000")
c.showPage()

header(c, 2, 3)
c.setFont("Helvetica", 9)
c.drawString(41.9, 647.1, "02.12")
c.drawString(268.6, 647.1, "ANCIEN SOLDE")
c.drawRightString(CREDIT_RIGHT, 647.1, "1 250,00")
table(c, PAGE_2, 623.2)
c.showPage()

header(c, 3, 3)
y = table(c, PAGE_3, 647.1)
c.drawString(92.8, y, "SOLDE INTERMEDIAIRE A FIN DECEMBRE")
c.drawRightString(CREDIT_RIGHT, y, "2 539,95")
y -= 23.9
c.drawString(316.5, y, "TOTAUX")
c.drawRightString(DEBIT_RIGHT, y, "1 140,05")
c.drawRightString(CREDIT_RIGHT, y, "3 660,00")
y -= 23.9
c.drawString(41.9, y, "02.01")
c.drawString(263.4, y, "SOLDE EN EUROS")
c.drawRightString(CREDIT_RIGHT, y, "2 519,95")
c.drawString(47.9, y - 72, "Votre autorisation de découvert au 15/12/2025.")
c.drawRightString(CREDIT_RIGHT, y - 82, "500,00")
c.showPage()
c.save()

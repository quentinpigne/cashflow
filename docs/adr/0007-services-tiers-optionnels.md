# ADR-0007 — Services tiers (OCR) explicites et optionnels

- **Statut** : Accepté
- **Date** : 2026-10-20
- **Concerne** : spec 03, architecture

## Contexte

Certains relevés n'existent qu'en PDF, et l'OCR par un service tiers (Mistral, par exemple) est le
moyen le plus fiable de les lire. Or un relevé contient l'identité, l'IBAN et toutes les dépenses du
titulaire, et l'appel au service nécessite une clé d'API, donc un secret à protéger.

## Décision

- Toute fonctionnalité qui envoie des données à un service tiers est **désactivée par défaut**,
  activée explicitement dans les réglages, et l'interface indique, au moment de l'action, que le
  document va être envoyé à tel fournisseur.
- L'OCR est derrière un **trait** (`DocumentTextExtractor`) : le fournisseur est interchangeable, y
  compris par une solution locale (Tesseract, modèle local) si elle devient suffisante.
- Les formats qui ne nécessitent aucun tiers (CSV, XLSX, OFX) sont prioritaires (spec 03).
- Pour les PDF, l'**extraction locale du texte** (texte et position de chaque élément, dans un PDF
  numérique) est tentée en premier. L'OCR par un tiers n'est qu'un **repli**, pour un PDF scanné ou en
  cas d'échec de l'extraction locale, et reste soumis à l'activation explicite.
- Les secrets (clés d'API) sont fournis par l'environnement ou un trousseau système, **jamais**
  versionnés. Le dépôt contient un fichier d'exemple sans valeur.

## Conséquences

- L'application est pleinement utilisable sans aucun service tiers.
- Le parseur LCL (V1) fonctionne sans OCR sur les relevés numériques téléchargés depuis l'espace client.
  Ce point est à vérifier sur un vrai relevé en phase 2. L'OCR n'est requis que pour un relevé scanné.
- Trait supplémentaire `PdfTextExtractor` (local) à côté de `DocumentTextExtractor` (OCR). Bibliothèques
  Rust à évaluer : `pdfium-render` (fiable, mais embarque la bibliothèque PDFium) ou `lopdf` (pur Rust,
  plus bas niveau).

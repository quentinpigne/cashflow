-- Onglet Simuler par projet (CRE-11, R-CRE-30) : la situation d'emprunteur quitte les hypothèses de la capacité
-- d'achat ; les conditions du crédit passent aux conditions par défaut de chaque projet (R-CRE-38) et au dernier
-- état de l'onglet (R-CRE-39), qui repartent de leurs valeurs initiales.
INSERT INTO app_setting (key, value, updated_at)
SELECT 'credit.borrower_situation',
       json_object(
           'max_debt_ratio_ppm', json_extract(value, '$.max_debt_ratio_ppm'),
           'reserve_months', json_extract(value, '$.reserve_months'),
           'account_ids', json(json_extract(value, '$.account_ids')),
           'other_income_minor', json_extract(value, '$.other_income_minor'),
           'other_charges_minor', json_extract(value, '$.other_charges_minor'),
           'co_borrower_income_minor', json_extract(value, '$.co_borrower_income_minor'),
           'co_borrower_charges_minor', json_extract(value, '$.co_borrower_charges_minor'),
           'excluded_loan_ids', json(json_extract(value, '$.excluded_loan_ids'))
       ),
       updated_at
FROM app_setting
WHERE key = 'credit.purchase_capacity'
ON CONFLICT (key) DO NOTHING;

DELETE FROM app_setting WHERE key = 'credit.purchase_capacity';

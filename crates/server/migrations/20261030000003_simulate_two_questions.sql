-- Onglet Simuler à deux questions (CRE-11, R-CRE-41 supprimée) : « Que coûte ce prêt ? » disparaît. Un dernier
-- état gardé sur cette question repart de « Que puis-je acheter ? » ; ses champs propres sont retirés.
UPDATE app_setting
SET value = json_remove(
        json_set(value, '$.question',
                 CASE json_extract(value, '$.question') WHEN 'LOAN' THEN 'MAX_PRICE'
                      ELSE json_extract(value, '$.question') END),
        '$.goal', '$.principal_minor')
WHERE key = 'credit.simulate_state';

-- Une ventilation est à un membre ou au foyer, le titulaire du compte par défaut (R-MBR-6) : les ventilations
-- sans membre d'un compte qui a un titulaire, hors virements, lui reviennent, ce qui garde les chiffres de chacun.
UPDATE split
SET member_id = (SELECT a.member_id FROM ledger_transaction t JOIN account a ON a.id = t.account_id
                 WHERE t.id = split.transaction_id)
WHERE member_id IS NULL
  AND id NOT IN (SELECT outflow_split_id FROM transfer UNION ALL SELECT inflow_split_id FROM transfer)
  AND EXISTS (SELECT 1 FROM ledger_transaction t JOIN account a ON a.id = t.account_id
              WHERE t.id = split.transaction_id AND a.member_id IS NOT NULL);

# Glossary (EN ⇄ ES-CL)

| Term | Español | Meaning in Mr Krabs |
|---|---|---|
| Account | Cuenta | Asset or liability holding money (checking, card, loan…). |
| Bank statement | Cartola / Estado de cuenta | Periodic document from the bank; *estado de cuenta* usually refers to credit cards. |
| Billing date | Fecha de facturación | Card statement cut-off day. |
| Due date | Fecha de pago / vencimiento | Day the card bill must be paid. |
| Installments | Cuotas | Purchase paid in N monthly parts. |
| Common expenses | Gastos comunes | Building/condo maintenance fee. |
| Rent / mortgage payment | Arriendo / Dividendo | Housing costs. |
| UF | Unidad de Fomento (CLF) | Inflation-indexed unit of account, 4 decimals. |
| RUT | Rol Único Tributario | Chilean tax ID (never stored in fixtures). |
| Emergency fund | Fondo de emergencia | Savings goal; contributions are transfers, not expenses. |
| Household | Hogar | The two members and their shared finances. |
| Scope | Ámbito | `personal` or `household`. |
| Settle-up | Saldar cuentas | Balancing who owes whom for shared expenses. |
| Journal entry | Asiento | A transaction with ≥2 postings. |
| Posting | Movimiento / línea | One leg of an entry (account or category). |
| HLC | Reloj lógico híbrido | Clock used to order sync changes. |
| Tombstone | Lápida | Soft-delete marker kept for sync. |
| Staging | Área de revisión | Imported rows awaiting approval. |
| Actual | Real | What a category really moved in a period: the net of its category legs by entry date (expenses minus refunds; income likewise), in the base currency. Transfers never count (DEC-29 Q7). |
| Planned (budget) | Planificado (presupuesto) | The amount set for a category in a month and scope (`budget_line`); one live line per slot (DEC-30 Q9). |
| Saved / invested | Ahorro e inversión | Net transfers from non-savings accounts into savings/investment-kind accounts in a period. Card payments are not saving (DEC-29 Q7). |
| Free cash flow | Flujo libre | Income − expenses − saved/invested for a period (DEC-29 Q7). |
| Savings rate | Tasa de ahorro | Saved/invested ÷ income, as integer basis points computed in Rust (DEC-29 Q7). |
| Roll-up | Consolidación (subtotal) | A parent category's planned and actual amounts include its own and all its descendants', at any depth (FR-BU-08, DEC-30 Q9). |
| Credit (in a category) | Crédito | An actual below zero: refunds larger than spending in the period. |

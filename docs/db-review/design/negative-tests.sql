-- Negative tests for schema.sql (+ schema-planned.sql). Synthetic data only.
-- Run: cat schema.sql schema-planned.sql negative-tests.sql | sqlite3 :memory:
-- Every statement after the "must FAIL" marker must be rejected; the last SELECT reports survivors.
.bail off
INSERT INTO currency VALUES ('CLP','Peso','$',0,'fiat',0,'seed',0),('USD','Dólar','US$',2,'fiat',0,'seed',0);
INSERT INTO household (id,name,base_currency_code,hlc,hlc_device) VALUES ('h','Casa','CLP',1,'a');
INSERT INTO member (id,household_id,display_name,hlc,hlc_device) VALUES ('m','h','A',1,'a');
INSERT INTO account (id,name,kind,currency_code,scope,owner_member_id,visibility,hlc,hlc_device) VALUES ('chk','Cuenta','checking','CLP','personal','m','shared_summary',1,'a');
INSERT INTO account (id,name,kind,currency_code,scope,visibility,hlc,hlc_device) VALUES ('sav','Ahorro','savings','CLP','household','shared_full',1,'a');
INSERT INTO category (id,parent_id,kind,name,name_key,seed_key,hlc,hlc_device) VALUES ('food',NULL,'expense','Comida','comida','expense.food',0,'seed');
INSERT INTO category (id,parent_id,kind,name,name_key,seed_key,hlc,hlc_device) VALUES ('super','food','expense','Supermercado','supermercado','expense.food.supermarket',0,'seed');
INSERT INTO category (id,parent_id,kind,name,name_key,hlc,hlc_device) VALUES ('deep','super','expense','Orgánico','organico',1,'a');  -- depth 3 is fine
INSERT INTO category (id,parent_id,kind,name,name_key,hlc,hlc_device) VALUES ('sal',NULL,'income','Sueldo','sueldo',1,'a');
INSERT INTO journal_entry (id,entry_date,hlc,hlc_device) VALUES ('e','2026-11-01',1,'a');
INSERT INTO posting (id,entry_id,leg,account_id,amount_minor,currency_code,status,hlc,hlc_device) VALUES ('p1','e','account','chk',-100,'CLP','cleared',1,'a');
INSERT INTO posting (id,entry_id,leg,category_id,amount_minor,currency_code,hlc,hlc_device) VALUES ('p2','e','category','food',100,'CLP',1,'a');   -- parent node takes postings
INSERT INTO posting (id,entry_id,leg,amount_minor,currency_code,hlc,hlc_device) VALUES ('p3','e','fx',5,'USD',1,'a');
INSERT INTO reconciliation (id,account_id,statement_date,statement_balance_minor,status,hlc,hlc_device) VALUES ('r','chk','2026-11-30',0,'open',1,'a');
INSERT INTO import_batch (id,file_name,file_sha256,extractor,status,created_at,hlc,hlc_device) VALUES ('b','x.xlsx','00','workbook','staged','2026-11-22',1,'a');
INSERT INTO rule (id,name,hlc,hlc_device) VALUES ('ru','Uber',1,'a');
INSERT INTO import_profile (id,name,format,parser_kind,bank,doc_type,hlc,hlc_device) VALUES ('pb','Banco X tarjeta','pdf','builtin','banco_x','card_statement',1,'a');
INSERT INTO import_profile (id,name,format,parser_kind,hlc,hlc_device) VALUES ('pw','Planilla casa','xlsx','workbook',1,'a');
INSERT INTO recurring_rule (id,name,kind,rrule,amount_mode,expected_minor,tolerance_pct,currency_code,next_date,hlc,hlc_device) VALUES ('rr','Luz','bill','FREQ=MONTHLY','approximate',30000,'10','CLP','2026-12-05',1,'a');
INSERT INTO recurring_rule_posting (id,rule_id,leg,account_id,amount_minor,currency_code,hlc,hlc_device) VALUES ('rp1','rr','account','chk',-30000,'CLP',1,'a');
INSERT INTO recurring_rule_posting (id,rule_id,leg,category_id,amount_minor,currency_code,hlc,hlc_device) VALUES ('rp2','rr','category','food',30000,'CLP',1,'a');
INSERT INTO recurring_rule_share (id,rule_id,member_id,amount_minor,currency_code,hlc,hlc_device) VALUES ('rs1','rr','m',15000,'CLP',1,'a');
SELECT 'positive ok: postings=' || (SELECT count(*) FROM posting) || ' categories=' || (SELECT count(*) FROM category);
SELECT '--- every statement below must FAIL';
INSERT INTO category (id,parent_id,kind,name,name_key,hlc,hlc_device) VALUES ('x01','food','expense','SUPERMERCADO','supermercado',1,'a');   -- sibling name_key dup
INSERT INTO category (id,parent_id,kind,name,name_key,seed_key,hlc,hlc_device) VALUES ('x02',NULL,'expense','X','x','expense.food',1,'a'); -- seed_key dup
INSERT INTO category (id,parent_id,kind,name,name_key,hlc,hlc_device) VALUES ('x03','food','income','Y','y',1,'a');           -- child kind <> parent kind
INSERT INTO category (id,parent_id,kind,name,name_key,hlc,hlc_device) VALUES ('x04','x04','expense','Z','z',1,'a');           -- self parent
INSERT INTO category (id,parent_id,kind,name,name_key,merged_into_id,hlc,hlc_device) VALUES ('x05',NULL,'expense','W','w','food',1,'a');  -- merged but live
INSERT INTO category (id,parent_id,kind,name,name_key,merged_into_id,deleted,hlc,hlc_device) VALUES ('x06',NULL,'income','V','v','food',1,1,'a'); -- merge across kinds
INSERT INTO posting (id,entry_id,leg,account_id,amount_minor,currency_code,status,hlc,hlc_device) VALUES ('x07','e','account','chk',1,'USD','cleared',1,'a');     -- leg currency <> account
INSERT INTO posting (id,entry_id,leg,account_id,amount_minor,currency_code,hlc,hlc_device) VALUES ('x08','e','account','chk',1,'CLP',1,'a');                    -- account leg w/o status
INSERT INTO posting (id,entry_id,leg,category_id,amount_minor,currency_code,status,hlc,hlc_device) VALUES ('x09','e','category','food',1,'CLP','cleared',1,'a'); -- status on category leg
INSERT INTO posting (id,entry_id,leg,account_id,amount_minor,currency_code,status,hlc,hlc_device) VALUES ('x10','e','fx','chk',1,'CLP','cleared',1,'a');        -- fx leg with a target
INSERT INTO posting (id,entry_id,leg,account_id,amount_minor,currency_code,status,hlc,hlc_device) VALUES ('x11','e','account','chk',1,'CLP','reconciled',1,'a');  -- reconciled w/o reconciliation
INSERT INTO posting (id,entry_id,leg,account_id,amount_minor,currency_code,status,reconciliation_id,hlc,hlc_device) VALUES ('x12','e','account','sav',1,'CLP','reconciled','r',1,'a'); -- other account's statement
INSERT INTO account (id,name,kind,currency_code,scope,visibility,credit_limit_minor,hlc,hlc_device) VALUES ('x13','A','checking','CLP','household','shared_full',100,1,'a'); -- card attr on checking
INSERT INTO journal_entry (id,entry_date,hlc,hlc_device) VALUES ('x14','2026-13-1',1,'a');                                                                        -- bad date
INSERT INTO payee_rule (id,match_kind,pattern,hlc,hlc_device) VALUES ('x15','contains','uber',1,'a');                                                             -- sets nothing
INSERT INTO rule_condition (id,rule_id,field,op,hlc,hlc_device) VALUES ('x16','ru','description','contains',1,'a');                                                -- condition w/o value
INSERT INTO rule_action (id,rule_id,action,hlc,hlc_device) VALUES ('x17','ru','set_category',1,'a');                                                               -- action w/o category
INSERT INTO account_valuation (account_id,as_of_date,value_minor,currency_code,source,hlc,hlc_device) VALUES ('chk','2026-11-30',1,'USD','manual',1,'a');          -- valuation currency <> account
INSERT INTO account (id,name,kind,currency_code,scope,owner_member_id,visibility,hlc,hlc_device) VALUES ('x19','P','prepaid','CLP','personal','m','private',1,'a');                       -- unknown account kind
INSERT INTO journal_entry (id,entry_date,kind,hlc,hlc_device) VALUES ('x20','2026-11-02','expense',1,'a');                                                         -- derived kind no longer storable
INSERT INTO import_profile (id,name,format,parser_kind,hlc,hlc_device) VALUES ('x21','B','pdf','builtin',1,'a');                                                   -- builtin without bank/doc
INSERT INTO import_profile (id,name,format,parser_kind,hlc,hlc_device) VALUES ('x22','M','csv','mapping',1,'a');                                                   -- mapping without mapping_json
INSERT INTO import_profile (id,name,format,parser_kind,hlc,hlc_device) VALUES ('x23','W','csv','workbook',1,'a');                                                  -- workbook not xlsx
INSERT INTO recurring_rule (id,name,kind,rrule,amount_mode,expected_minor,currency_code,next_date,hlc,hlc_device) VALUES ('x24','A','bill','R','approximate',1,'CLP','2026-12-01',1,'a'); -- approx w/o tolerance
INSERT INTO recurring_rule (id,name,kind,rrule,amount_mode,expected_min_minor,expected_max_minor,currency_code,next_date,hlc,hlc_device) VALUES ('x25','B','bill','R','range',9,1,'CLP','2026-12-01',1,'a'); -- min > max
INSERT INTO recurring_rule (id,name,kind,rrule,amount_mode,expected_minor,next_date,hlc,hlc_device) VALUES ('x26','C','bill','R','exact',1,'2026-12-01',1,'a');      -- amount w/o currency
INSERT INTO recurring_rule_posting (id,rule_id,leg,account_id,amount_minor,currency_code,hlc,hlc_device) VALUES ('x27','rr','account','chk',1,'USD',1,'a');       -- template leg currency <> account
INSERT INTO recurring_rule_share (id,rule_id,member_id,participant_id,amount_minor,currency_code,hlc,hlc_device) VALUES ('x28','rr',NULL,NULL,1,'CLP',1,'a');    -- share with no holder
SELECT '--- end; postings=' || (SELECT count(*) FROM posting) || ' categories=' || (SELECT count(*) FROM category) || ' rule_conditions=' || (SELECT count(*) FROM rule_condition) || ' profiles=' || (SELECT count(*) FROM import_profile) || ' recurring=' || (SELECT count(*) FROM recurring_rule) || ' tmpl_legs=' || (SELECT count(*) FROM recurring_rule_posting);

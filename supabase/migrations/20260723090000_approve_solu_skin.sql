begin;

-- Solu is the only official v1 beta pet that has completed the atlas and
-- desktop QA pipeline. Projection publication still falls back to Yoonie for
-- every other unapproved/custom id.
insert into private.approved_skins (skin_id)
values ('solu')
on conflict (skin_id) do nothing;

commit;

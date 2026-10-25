import assert from 'node:assert/strict';
import test from 'node:test';
import { patchLabel, patchMenuItems, factoryItems, userItems, neighbour, type PatchActions } from './patchMenu.ts';
import { SYNTH_META } from './synth-meta.ts';
import { createPreviewSet } from './model.ts';
const refs=SYNTH_META.factory.map(({id,name,category})=>({id,name,category}));
const actions:PatchActions={load(){},save(){},saveAs(){},rename(){},remove(){},revert(){},showFolder(){},openFactory(){},openUser(){}};
test('patch LCD marks only modified patches',()=>{const k=createPreviewSet().keys;assert.equal(patchLabel(k),'E.PIANO');k.modified=true;assert.equal(patchLabel(k),'E.PIANO *');});
test('patch menu protects factory names and enables user operations',()=>{
 const k=createPreviewSet().keys;
 let items=patchMenuItems(k,0,actions);for(const label of ['RENAME…','DELETE','REVERT','USER ▸'])assert.equal(items.find(i=>i.label===label)?.disabled,true,label);
 k.patchSource={user:'MY PATCH.lbpatch'};k.modified=true;items=patchMenuItems(k,1,actions);for(const label of ['RENAME…','DELETE','REVERT','USER ▸'])assert.equal(items.find(i=>i.label===label)?.disabled,false,label);
});
test('factory menu groups all sixteen patches and checks only the current source',()=>{
 const items=factoryItems(refs,{factory:'organ'},()=>{});assert.deepEqual(items.filter(i=>i.info).map(i=>i.label),['PIANO','KEYS','ORGAN','PAD','LEAD','BASS','PLUCK']);assert.equal(items.filter(i=>!i.info).length,16);assert.deepEqual(items.filter(i=>i.checked).map(i=>i.label),['ORGAN']);
});
test('user patches are alphabetical without changing the input list',()=>{const list=[{id:'Z.lbpatch',name:'Z',category:'keys' as const},{id:'A.lbpatch',name:'A',category:'keys' as const}];assert.deepEqual(userItems(list,'init',()=>{}).map(i=>i.label),['A','Z']);assert.equal(list[0].name,'Z');});
test('patch neighbours follow factory then sorted user patches and wrap',()=>{
 const lists={factory:refs,user:[{id:'Z.lbpatch',name:'Z',category:'keys' as const},{id:'A.lbpatch',name:'A',category:'keys' as const}]};
 assert.deepEqual(neighbour(lists,{factory:refs.at(-1)!.id},1),{user:'A.lbpatch'});assert.deepEqual(neighbour(lists,{factory:refs[0].id},-1),{user:'Z.lbpatch'});assert.deepEqual(neighbour(lists,'init',1),{factory:refs[0].id});assert.equal(neighbour({factory:[],user:[]},'init',1),null);
});

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { hitTest, dropTag, physicalToCss, isAudioName, passedThreshold } from './dnd.ts';
test('drop hit testing is ordered and uses half-open edges', () => {
  const rect={left:10,top:20,width:30,height:40};
  const pads=[{x:1,y:2,rect}],banks=[{bank:3,rect:{...rect,left:60}}],editor={...rect,left:100};
  assert.deepEqual(hitTest(10,20,pads,banks,editor),{kind:'pad',x:1,y:2});
  assert.equal(hitTest(40,20,pads,banks,editor),null); assert.equal(hitTest(10,60,pads,banks,editor),null);
  assert.deepEqual(hitTest(60,20,pads,banks,editor),{kind:'bank',bank:3});
  assert.deepEqual(hitTest(100,20,pads,banks,editor),{kind:'editor'});
  assert.equal(hitTest(0,0,pads,banks,editor),null);
  assert.deepEqual(hitTest(10,20,pads,[{bank:0,rect}],rect),{kind:'pad',x:1,y:2});
});
test('drop tags cover import replace move swap and live copy', () => {
  assert.equal(dropTag('files',false,false,1),'IMPORT'); assert.equal(dropTag('files',true,false,1),'REPLACE');
  assert.equal(dropTag('files',true,true,3),'IMPORT ×3'); assert.equal(dropTag('pad',false,false,1),'MOVE');
  assert.equal(dropTag('pad',true,false,1),'SWAP'); assert.equal(dropTag('pad',true,true,1),'COPY');
  assert.equal(dropTag('pad',false,true,1),'COPY');
});
test('native coordinates, audio extensions and drag threshold', () => {
  assert.deepEqual(physicalToCss({x:300,y:150},1.5),{x:200,y:100});
  assert.ok(isAudioName('A.FLAC')); assert.ok(!isAudioName('readme.md'));
  assert.ok(!passedThreshold(3,0)); assert.ok(passedThreshold(3,3));
});

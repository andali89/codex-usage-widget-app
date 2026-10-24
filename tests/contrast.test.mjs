import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const css=readFileSync(new URL("../src/styles.css",import.meta.url),"utf8");
function palette(selector) {
  const block=css.match(new RegExp(`${selector.replace(/[.*+?^${}()|[\]\\]/g,"\\$&")}\\s*\\{([^}]+)\\}`))?.[1];
  assert.ok(block,`Palette exists: ${selector}`);
  return Object.fromEntries([...block.matchAll(/--([\w-]+):\s*(#[\da-f]{3,6})\s*;/gi)].map(([,key,value])=>[key,value]));
}
function luminance(hex) {
  const full=hex.length===4?hex.slice(1).split("").map(c=>c+c).join(""):hex.slice(1);
  const linear=[0,2,4].map(offset=>parseInt(full.slice(offset,offset+2),16)/255).map(c=>c<=0.04045?c/12.92:((c+0.055)/1.055)**2.4);
  return linear[0]*0.2126+linear[1]*0.7152+linear[2]*0.0722;
}
const contrast=(a,b)=>{const x=luminance(a),y=luminance(b);return (Math.max(x,y)+0.05)/(Math.min(x,y)+0.05);};

for(const [name,selector] of [["Light",":root"],["Dark",':root[data-theme="dark"]']]) {
  test(`${name} semantic text, controls, focus and quota graphics meet specified contrast`,()=>{
    const p=palette(selector);
    for(const foreground of ["text","secondary","accent","weekly","healthy","warning","critical"]){
      for(const background of ["canvas","surface","selected","wash"]){
        const ratio=contrast(p[foreground],p[background]);
        assert.ok(ratio>=4.5,`${foreground} on ${background}: ${ratio.toFixed(2)}:1 < 4.5:1`);
      }
    }
    for(const foreground of ["border","accent"]){
      for(const background of ["canvas","surface","selected"]){
        const ratio=contrast(p[foreground],p[background]);assert.ok(ratio>=3,`${foreground} on ${background}: ${ratio.toFixed(2)}:1 < 3:1`);
      }
    }
    for(const foreground of ["accent","weekly"]){
      const ratio=contrast(p[foreground],p.track);assert.ok(ratio>=3,`${foreground} quota bar on track: ${ratio.toFixed(2)}:1 < 3:1`);
    }
  });
}

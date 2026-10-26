import { useLayoutEffect, useRef, type RefObject } from "react";

const ns="http://www.w3.org/2000/svg";
// Only background surfaces participate. Quota graphics, tracks, text, borders,
// focus outlines, status dots, and artwork remain in the opaque foreground.
const surfaces=".quota-card,.widget-credits,.widget-header,.widget-status,.widget-empty,.edge-values,.hud-values,.status-warning,.period-badge,.icon-button";

/** Flatten opaque surface paints before applying alpha once to the SVG group. */
export default function WidgetBackground({root,alpha}:{root:RefObject<HTMLDivElement|null>;alpha:number}) {
  const layer=useRef<SVGSVGElement>(null);
  useLayoutEffect(()=>{
    const host=root.current,svg=layer.current;
    if(!host||!svg)return;
    let frame=0;
    const elements=[host,...host.querySelectorAll<HTMLElement>(surfaces)];
    const clear=()=>elements.forEach(element=>element.removeAttribute("data-surface-paint"));
    const paint=()=>{
      clear();
      const bounds=host.getBoundingClientRect(),hostStyle=getComputedStyle(host);
      const left=parseFloat(hostStyle.borderLeftWidth),top=parseFloat(hostStyle.borderTopWidth);
      const width=bounds.width-left-parseFloat(hostStyle.borderRightWidth),height=bounds.height-top-parseFloat(hostStyle.borderBottomWidth);
      svg.setAttribute("viewBox",`0 0 ${width} ${height}`);
      svg.style.width=`${width}px`;svg.style.height=`${height}px`;
      const defs=document.createElementNS(ns,"defs"),group=document.createElementNS(ns,"g");
      group.setAttribute("opacity",String(alpha));
      elements.forEach((element,index)=>{
        const style=getComputedStyle(element),r=element.getBoundingClientRect();
        if(!r.width||!r.height)return;
        const x=element===host?0:r.left-bounds.left-left,y=element===host?0:r.top-bounds.top-top;
        const w=element===host?width:r.width,h=element===host?height:r.height;
        const shape=(fill:string)=>{
          const rect=document.createElementNS(ns,"rect");
          for(const [key,value] of Object.entries({x,y,width:w,height:h,rx:Math.max(0,parseFloat(style.borderTopLeftRadius)-(element===host?left:0))}))rect.setAttribute(key,String(value));
          rect.setAttribute("fill",fill);rect.dataset.paintSource=element===host?"shell":element.className;
          group.append(rect);
        };
        if(style.backgroundColor!=="rgba(0, 0, 0, 0)"&&style.backgroundColor!=="transparent")shape(style.backgroundColor);
        if(style.backgroundImage!=="none") {
          // Current widget surfaces use two-stop linear gradients, with opaque
          // colors. Keep their angle and endpoints in the flattened paint layer.
          const colors=style.backgroundImage.match(/rgba?\([^)]+\)/g)??[];
          if(colors.length>=2) {
            const gradient=document.createElementNS(ns,"linearGradient"),id=`surface-gradient-${index}`;
            gradient.id=id;gradient.setAttribute("gradientUnits","userSpaceOnUse");
            const angle=parseFloat(style.backgroundImage.match(/linear-gradient\(([-.\d]+)deg/)?.[1]??"180")*Math.PI/180;
            const dx=Math.sin(angle),dy=-Math.cos(angle),length=Math.abs(w*dx)+Math.abs(h*dy);
            for(const [key,value] of Object.entries({x1:x+w/2-dx*length/2,y1:y+h/2-dy*length/2,x2:x+w/2+dx*length/2,y2:y+h/2+dy*length/2}))gradient.setAttribute(key,String(value));
            colors.forEach((color,i)=>{const stop=document.createElementNS(ns,"stop");stop.setAttribute("offset",String(i/(colors.length-1)));stop.setAttribute("stop-color",color);gradient.append(stop);});
            defs.append(gradient);shape(`url(#${id})`);
          }
        }
      });
      svg.replaceChildren(defs,group);
      elements.forEach(element=>element.setAttribute("data-surface-paint",""));
    };
    const schedule=()=>{cancelAnimationFrame(frame);frame=requestAnimationFrame(paint);};
    paint();
    const observer=new ResizeObserver(schedule);elements.forEach(element=>observer.observe(element));
    for(const event of ["pointerover","pointerout","focusin","focusout","transitionend"])host.addEventListener(event,schedule);
    return ()=>{cancelAnimationFrame(frame);observer.disconnect();clear();for(const event of ["pointerover","pointerout","focusin","focusout","transitionend"])host.removeEventListener(event,schedule);};
  });
  return <svg ref={layer} className="surface-background" aria-hidden="true" focusable="false"/>;
}

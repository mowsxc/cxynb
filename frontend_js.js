<script>
// ═══ 核心财务公式（严禁修改逻辑）═══
// 实收金额 = 消费金额(sale_price) - 商家优惠(discount)
// 服务费   = 实收金额 × 7%
// 财务价   = 实收金额 - 服务费
// 计费价   = 按关键词匹配计费规则
const API='';

let latestDbDate='';
let refundFilter='normal';
let activeShift='';
let sortCol='consume_date';
let sortDir='desc';

// ══════════════════════════════════════════
//  计费价配置表
// ══════════════════════════════════════════
function getFeePlans(){
  const saved=localStorage.getItem('feePlans');
  if(saved){
    try{
      const plans=JSON.parse(saved);
      if(Array.isArray(plans)&&plans.length>0) return plans;
    }catch(e){}
  }
  // 尝试从后端加载
  if(window._feePlansCache&&window._feePlansCache.length>0) return window._feePlansCache;
  // 默认规则
  return DEFAULT_FEE_PLANS;
}
const DEFAULT_FEE_PLANS=[
  {cat:'新会员',plan:'特惠',fee:30},{cat:'新会员',plan:'女神',fee:30},{cat:'新会员',plan:'超值',fee:100},
  {cat:'5070显卡',plan:'3小时',fee:34},{cat:'5070显卡',plan:'4小时',fee:44},{cat:'5070显卡',plan:'包天',fee:110},
  {cat:'网游区',plan:'3小时',fee:26},{cat:'网游区',plan:'4小时',fee:36},{cat:'网游区',plan:'包天',fee:90},
  {cat:'网游区',plan:'包早',fee:25},{cat:'网游区',plan:'包夜',fee:45},
  {cat:'普通区',plan:'包夜',fee:30},{cat:'普通区',plan:'包天',fee:70},
  {cat:'老会员',plan:'生日',fee:66},{cat:'电竞区5070',plan:'通宵',fee:55},
  {cat:'1000网费',plan:'送500',fee:1000},{cat:'100网费',plan:'送20',fee:100},
];

function calcFee(product_info){
  if(!product_info) return {fee:0,label:''};
  for(const p of getFeePlans()){
    if(p.cat && p.plan && product_info.includes(p.cat)&&product_info.includes(p.plan)){
      return {fee:p.fee,label:p.cat+' '+p.plan};
    }
  }
  return {fee:0,label:''};
}
/**
 * 财务价 = 消费金额 - 商家优惠 - 服务费7%
 */
function calcFinancial(salePriceStr, discountStr){
  const sale = parseFloat(String(salePriceStr||'0').replace(/[¥￥,]/g,''))||0;
  const disc = getTotalDiscount(discountStr);
  const actual = sale - disc; // 实收金额 = 消费金额 - 商家优惠
  const fee = actual * 0.07; // 服务费 = 实收金额×7%
  return actual - fee; // 财务价 = 实收金额 - 服务费
}
let curPage=1;
const pageSize=100;
let curTotal=0;
let allRows=[];
let autoTimer=null;

// === 列显示设置 ===
const COLUMNS = [
  {id:'product_info', label:'交易快照', pc:true, mob:true},
  {id:'product_type', label:'类型', pc:true, mob:false},
  {id:'coupon_value', label:'券号', pc:true, mob:true},
  {id:'sale_price', label:'消费金额', pc:true, mob:false},
  {id:'discount_price', label:'商家优惠', pc:true, mob:false},
  {id:'consume_date', label:'消费时间', pc:true, mob:true},
  {id:'mobile', label:'用户手机', pc:true, mob:false},
  {id:'description', label:'备注', pc:true, mob:false},
  {id:'shop_info', label:'验证门店', pc:true, mob:false},
  {id:'fee', label:'计费价', pc:true, mob:true},
  {id:'financial', label:'财务价', pc:false, mob:false},
];
function isMobile(){return window.innerWidth<768||/Mobi|Android|iPhone/i.test(navigator.userAgent);}
function loadColumnSettings(){
  const s=localStorage.getItem('columnSettings');
  if(s) {
    try {
      const parsed = JSON.parse(s);
      if (parsed && Array.isArray(parsed.columns)) {
        parsed.columns.forEach(col => {
          if (col.copy === undefined) {
            col.copy = col.vis !== false;
          }
        });
        if (parsed.copyHeader === undefined) {
          parsed.copyHeader = false;
        }
        return parsed;
      }
    } catch(e){}
  }
  return {
    keyInfo: isMobile(),
    copyHeader: false,
    columns: COLUMNS.map(c => ({
      id: c.id,
      vis: isMobile() ? c.mob : c.pc,
      copy: (c.id === 'fee' || c.id === 'financial') ? false : (isMobile() ? c.mob : c.pc)
    }))
  };
}
let columnSettings = loadColumnSettings();
function getProductName(pi){
  if(!pi) return '';
  return pi.replace(/\[[\d.]+元?\]/g,'').replace(/\[\d+\]/g,'').trim();
}
function getTotalDiscount(dp){
  if(!dp||dp==='-') return 0;
  const nums=(dp.match(/[:：](\d+\.\d+)/g)||[]);
  return nums.reduce((s,n)=>s+parseFloat(n.replace(/[：:]/g,'')),0);
}

function fmtEmpty(msg){return '<tr><td colspan="99" class="empty">'+msg+'</td></tr>';}
function pad(n){return String(n).padStart(2,'0')}
function fmt(d){return d.getFullYear()+'-'+pad(d.getMonth()+1)+'-'+pad(d.getDate())+' '+pad(d.getHours())+':'+pad(d.getMinutes())+':'+pad(d.getSeconds())}
function fmtLocal(d){return d.getFullYear()+'-'+pad(d.getMonth()+1)+'-'+pad(d.getDate())+'T'+pad(d.getHours())+':'+pad(d.getMinutes())}
function fmtDate(d){return d.getFullYear()+'-'+pad(d.getMonth()+1)+'-'+pad(d.getDate())}
function fmtTime(d){return pad(d.getHours())+':'+pad(d.getMinutes())}
function toAPI(v){return v?v.replace('T',' '):''}
function getDT(w){const d=document.getElementById(w+'Date').value, t=document.getElementById(w+'Time').value; return d?d+' '+t:''}
function esc(s){return String(s||'').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;').replace(/'/g,'&#39;')}
function fmtMoney(s){if(!s)return'';const n=parseFloat(String(s).replace(/[¥￥,]/g,''));return isNaN(n)?esc(s):'¥'+n.toLocaleString('zh-CN',{minimumFractionDigits:2})}
function maskPhone(p){if(!p)return'';if(p.length===11)return p.slice(0,3)+'****'+p.slice(7);return p}

// 全局优雅 Toast
function showToast(msg, type='info') {
  const container = document.getElementById('toastContainer');
  const toast = document.createElement('div');
  toast.className = `toast toast-${type}`;
  
  let icon = 'ℹ️';
  if(type==='success') icon = '✅';
  if(type==='warning') icon = '⚠️';
  if(type==='error') icon = '❌';

  toast.innerHTML = `<span>${icon}</span><span style="flex-grow:1">${esc(msg)}</span>`;
  container.appendChild(toast);
  
  setTimeout(() => {
    toast.remove();
  }, 3000);
}

function setShift(t){
  activeShift=t;
  const ss=window._shiftSet;
  document.getElementById('shiftDay').textContent='白班';
  document.getElementById('shiftNight').textContent='晚班';
  ['shiftDay','shiftNight','shiftToday','shiftYesterday','shiftMonth','shiftAll'].forEach(id=>{
    const el=document.getElementById(id);
    el.className='tag'+(id==='shift'+t.charAt(0).toUpperCase()+t.slice(1)?' on':'');
  });
  const n=new Date();let s,e;
  const isDay=n.getHours()>=8&&n.getHours()<20;
	  if(t==='day'){
	    const h=ss?ss.dayS[0]:8, m=ss?ss.dayS[1]:0, he=ss?ss.dayE[0]:20;
	    if(isDay){
	      s=new Date(n.getFullYear(),n.getMonth(),n.getDate(),h,m,0);
	      e=n; // 正在过白班，结束时间就是当前最新时间
	    }else if(n.getHours()>=20){
	      // 20点后，今天白班已结束
	      s=new Date(n.getFullYear(),n.getMonth(),n.getDate(),h,m,0);
	      e=new Date(n.getFullYear(),n.getMonth(),n.getDate(),he,0,0);
	    }else{
	      // 8点前，今天白班还没开始 → 昨天白班
	      s=new Date(n.getFullYear(),n.getMonth(),n.getDate()-1,h,m,0);
	      e=new Date(n.getFullYear(),n.getMonth(),n.getDate()-1,he,0,0);
	    }
	  }
  else if(t==='night'){
    const h=ss?ss.nightS[0]:20, m=ss?ss.nightS[1]:0, he=ss?ss.nightE[0]:8;
    if(!isDay){
      // 晚班时段
      s=n.getHours()<he
        ? new Date(n.getFullYear(),n.getMonth(),n.getDate()-1,h,m,0)
        : new Date(n.getFullYear(),n.getMonth(),n.getDate(),h,m,0);
      e=n;
    }else{
      // 白班时段，晚班跨天
      s=new Date(n.getFullYear(),n.getMonth(),n.getDate()-1,h,m,0);
      e=new Date(n.getFullYear(),n.getMonth(),n.getDate(),he,0,0);
    }
  }
  else if(t==='today'){s=new Date(n.getFullYear(),n.getMonth(),n.getDate(),0,0,0);e=n}
  else if(t==='yesterday'){s=new Date(n.getFullYear(),n.getMonth(),n.getDate()-1,0,0,0);e=new Date(n.getFullYear(),n.getMonth(),n.getDate()-1,23,59,59)}
  else if(t==='month'){
    const mh=ss&&ss.monthCal?0:8;
    s=new Date(n.getFullYear(),n.getMonth(),1,mh,0,0);
    if(ss&&ss.monthEndPrev){
      // 不含本班：截止到上个班次结束
      const h=n.getHours();
      if(h>=8&&h<20) e=new Date(n.getFullYear(),n.getMonth(),n.getDate(),8,0,0);
      else if(h<8) e=new Date(n.getFullYear(),n.getMonth(),n.getDate()-1,20,0,0);
      else e=new Date(n.getFullYear(),n.getMonth(),n.getDate(),20,0,0);
    }else{
      e=n;
    }
  }
  else{s=new Date(2026,0,1,0,0,0);e=n}
  document.getElementById('startDate').value=fmtDate(s);
  document.getElementById('startTime').value=fmtTime(s);
  document.getElementById('endDate').value=fmtDate(e);
  document.getElementById('endTime').value=fmtTime(e);
  query();
}

function clearShift(){
  activeShift='';
  ['shiftDay','shiftNight','shiftToday','shiftYesterday','shiftMonth','shiftAll'].forEach(id=>{
    document.getElementById(id).className='tag';
  });
}

// 统一设置面板
// 统一设置面板
function openSettings(){
  // 从后端加载业务规则（班次+计费）
  fetch(API+'/api/settings').then(r=>r.json()).then(s=>{
    if(s){
      // 加载列显示偏好（本地）
      if(s.columns) columnSettings={keyInfo:s.key_info||false, copyHeader:s.copy_header||false, columns:s.columns};
      renderColumnChecks();
      // 加载班次设置（后端）
      if(s.shift){
        const pad2=n=>String(n).padStart(2,'0');
        document.getElementById('setDayStart').value=pad2(s.shift.day_start[0])+':'+pad2(s.shift.day_start[1]);
        document.getElementById('setDayEnd').value=pad2(s.shift.day_end[0])+':'+pad2(s.shift.day_end[1]);
        document.getElementById('setNightStart').value=pad2(s.shift.night_start[0])+':'+pad2(s.shift.night_start[1]);
        document.getElementById('setNightEnd').value=pad2(s.shift.night_end[0])+':'+pad2(s.shift.night_end[1]);
        window._shiftSet={dayS:s.shift.day_start,dayE:s.shift.day_end,nightS:s.shift.night_start,nightE:s.shift.night_end};
      }
      // 加载本月统计边界
      const mStart = s.month_start_cal ? 'cal' : 'shift';
      const mEnd = s.month_end_prev ? 'prev' : 'now';
      const rStart = document.querySelector(`input[name="setMonthStart"][value="${mStart}"]`);
      if(rStart) rStart.checked = true;
      const rEnd = document.querySelector(`input[name="setMonthEnd"][value="${mEnd}"]`);
      if(rEnd) rEnd.checked = true;
    }
  }).catch(()=>{});
  // 班次设置的 localStorage 兜底
  const saved=localStorage.getItem('shiftSettings');
  if(saved){
    try{const ss=JSON.parse(saved);if(!window._shiftSet) window._shiftSet=ss;}catch(e){}
  }
  document.getElementById('setModal').style.display='block';
}
function applySettings(){
  // 保存班次设置到后端
  const dayS=document.getElementById('setDayStart').value.split(':').map(Number);
  const dayE=document.getElementById('setDayEnd').value.split(':').map(Number);
  const nightS=document.getElementById('setNightStart').value.split(':').map(Number);
  const nightE=document.getElementById('setNightEnd').value.split(':').map(Number);
  window._shiftSet={dayS,dayE,nightS,nightE};
  
  const mStart = document.querySelector('input[name="setMonthStart"]:checked').value;
  const mEnd = document.querySelector('input[name="setMonthEnd"]:checked').value;
  const monthSettings = {monthCal: mStart === 'cal', monthEndPrev: mEnd === 'prev'};
  
  // 收集列设置
  const cols = COLUMNS.map(c => {
    const visChk = document.querySelector(`input[data-col-vis="${c.id}"]`);
    const copyChk = document.querySelector(`input[data-col-copy="${c.id}"]`);
    return {
      id: c.id,
      vis: visChk ? visChk.checked : true,
      copy: copyChk ? copyChk.checked : true
    };
  });
  
  columnSettings = {
    keyInfo: document.getElementById('setKeyInfo').checked,
    copyHeader: document.getElementById('setCopyHeader').checked,
    columns: cols
  };
  
  // 同步存后端
  fetch(API+'/api/settings').then(r=>r.json()).then(serverSettings=>{
    serverSettings.shift={day_start:dayS,day_end:dayE,night_start:nightS,night_end:nightE};
    serverSettings.key_info=columnSettings.keyInfo;
    serverSettings.copy_header=columnSettings.copyHeader;
    serverSettings.columns=cols;
    serverSettings.month_start_cal = mStart === 'cal';
    serverSettings.month_end_prev = mEnd === 'prev';
    fetch(API+'/api/settings',{method:'PUT',headers:{'Content-Type':'application/json'},body:JSON.stringify(serverSettings)});
  });
  
  localStorage.setItem('shiftSettings',JSON.stringify(window._shiftSet));
  localStorage.setItem('monthSettings',JSON.stringify(monthSettings));
  localStorage.setItem('columnSettings',JSON.stringify(columnSettings));
  
  document.getElementById('shiftDay').textContent='白班';
  document.getElementById('shiftNight').textContent='晚班';
  closeModal('setModal');
  
  const curStart=document.getElementById('startDate').value;
  if(curStart&&activeShift) setShift(activeShift);
  else query();
}

// 列显示与复制设置渲染
function renderColumnChecks(){
  const box=document.getElementById('columnSettingsTable');
  if(!box)return;
  box.innerHTML=COLUMNS.map(c=>{
    const colCfg=columnSettings.columns.find(x=>x.id===c.id) || {vis:true, copy:true};
    const visChk=colCfg.vis!==false?'checked':'';
    const copyChk=colCfg.copy!==false?'checked':'';
    return `
    <div style="display:flex; align-items:center; padding:4px 8px; font-size:12px; border-bottom:1px solid #f3f4f6">
      <div style="flex:1; font-weight:500; color:var(--text)">${c.label}</div>
      <div style="width:70px; display:flex; justify-content:center">
        <input type="checkbox" class="ios-switch" data-col-vis="${c.id}" ${visChk}>
      </div>
      <div style="width:70px; display:flex; justify-content:center">
        <input type="checkbox" class="ios-switch" data-col-copy="${c.id}" ${copyChk}>
      </div>
    </div>
    `;
  }).join('');
  const ki=document.getElementById('setKeyInfo');
  if(ki) ki.checked=columnSettings.keyInfo;
  const sch=document.getElementById('setCopyHeader');
  if(sch) sch.checked=columnSettings.copyHeader || false;
}

function saveColumnSettings(){
  columnSettings.keyInfo = document.getElementById('setKeyInfo')?.checked || false;
  columnSettings.copyHeader = document.getElementById('setCopyHeader')?.checked || false;
  localStorage.setItem('columnSettings', JSON.stringify(columnSettings));
  // 同步到后端
  fetch(API+'/api/settings').then(r=>r.json()).then(s=>{
    s.key_info = columnSettings.keyInfo;
    s.copy_header = columnSettings.copyHeader;
    s.columns = columnSettings.columns.clone();
    fetch(API+'/api/settings',{method:'PUT',headers:{'Content-Type':'application/json'},body:JSON.stringify(s)});
  }).catch(()=>{});
}
}

function toggleKeyInfo(on){
  columnSettings.keyInfo=on;
  saveColumnSettings();
  query();
}

function syncCopyCols(){
  COLUMNS.forEach(c => {
    const visChk = document.querySelector(`input[data-col-vis="${c.id}"]`);
    const copyChk = document.querySelector(`input[data-col-copy="${c.id}"]`);
    if(visChk && copyChk) {
      copyChk.checked = visChk.checked;
    }
  });
  showToast('复制列已与大盘显示列同步！', 'success');
}

function setColDefaults(mode){
  const isMob=mode==='mob';
  COLUMNS.forEach(c => {
    const visChk = document.querySelector(`input[data-col-vis="${c.id}"]`);
    const copyChk = document.querySelector(`input[data-col-copy="${c.id}"]`);
    const defaultVal = isMob ? c.mob : c.pc;
    if(visChk) visChk.checked = defaultVal;
    if(copyChk) copyChk.checked = (c.id === 'fee' || c.id === 'financial') ? false : defaultVal;
  });
  const ki=document.getElementById('setKeyInfo');
  if(ki) ki.checked = isMob;
}

function openFeeConfig(){
  const plans=getFeePlans();
  let h='<div class="fee-card" style="overflow-y:auto">'+
    '<table class="fee-table" style="min-width:0">'+
    '<thead><tr>'+
    '<th>关键词1</th>'+
    '<th>关键词2</th>'+
    '<th>计费价</th>'+
    '<th style="text-align:center"></th></tr></thead><tbody>';
  plans.forEach((p,i)=>{
    h+='<tr>'+
      '<td><input type="text" id="fc_cat_'+i+'" value="'+esc(p.cat)+'" title="'+esc(p.cat)+'"></td>'+
      '<td><input type="text" id="fc_plan_'+i+'" value="'+esc(p.plan)+'" title="'+esc(p.plan)+'"></td>'+
      '<td class="fee-cell"><input type="text" id="fc_fee_'+i+'" value="'+p.fee+'"></td>'+
      '<td class="op-cell"><button type="button" class="del" onclick="removeFeeRow('+i+')" title="删除">✕</button></td></tr>';
  });
  h+='</tbody></table></div>';
  document.getElementById('feeConfigList').innerHTML=h;
  window._feeCount=plans.length;
  document.getElementById('feeModal').style.display='block';
}
function saveFeeConfig(){
  const plans=[];
  for(let i=0;i<window._feeCount;i++){
    const el=document.getElementById('fc_cat_'+i);
    if(!el||el.closest('tr').style.display==='none') continue;
    const cat=el.value.trim();
    const plan=document.getElementById('fc_plan_'+i).value.trim();
    const fee=parseFloat(document.getElementById('fc_fee_'+i).value);
    if(cat&&plan&&!isNaN(fee)) plans.push({cat,plan,fee});
  }
  localStorage.setItem('feePlans',JSON.stringify(plans));
  // 同步到后端配置
  const feeJson=JSON.stringify(plans);
  fetch(API+'/api/settings').then(r=>r.json()).then(s=>{
    s.fee_json=feeJson;
    fetch(API+'/api/settings',{method:'PUT',headers:{'Content-Type':'application/json'},body:JSON.stringify(s)});
  });
  closeModal('feeModal');
  showToast('计费配置已保存（'+plans.length+'项）','success');
  if(activeShift) setShift(activeShift); else query();
}
function addFeeRow(){
  const i=window._feeCount++;
  const tb=document.querySelector('#feeConfigList table.fee-table tbody');
  const tr=document.createElement('tr');
  tr.innerHTML=''+
    '<td><input type="text" id="fc_cat_'+i+'" value="" placeholder="关键词1"></td>'+
    '<td><input type="text" id="fc_plan_'+i+'" value="" placeholder="关键词2"></td>'+
    '<td class="fee-cell"><input type="text" id="fc_fee_'+i+'" value="0"></td>'+
    '<td class="op-cell"><button type="button" class="del" onclick="removeFeeRow('+i+')" title="删除">✕</button></td>';
  tb.appendChild(tr);
}
function removeFeeRow(i){
  const el=document.getElementById('fc_cat_'+i);
  if(el) el.closest('tr').remove();
}

	function setRefund(v){
  refundFilter=v;
  [['fAll','all'],['fNormal','normal'],['fRefund','refunded']].forEach(([id,val])=>{
    const el=document.getElementById(id);
    el.className='tag';
    if(v===val){
      if(val==='refunded')el.classList.add('on','on-red');
      else if(val==='normal')el.classList.add('on','on-green');
      else el.classList.add('on');
    }
  });
  query();
}

function resetFilters(){
  document.getElementById('product').value='';
  document.getElementById('coupon').value='';
  document.getElementById('mobile').value='';
  setRefund('all');
}

function sortBy(col){
  if(sortCol===col){sortDir=sortDir==='asc'?'desc':'asc'}
  else{sortCol=col;sortDir='desc'}
  document.querySelectorAll('.arrow').forEach(e=>e.textContent='');
  const a=document.getElementById('arrow-'+col);
  if(a)a.textContent=sortDir==='asc'?'▲':'▼';
  renderTable();
}

async function loadStats(){
  try{
    const r=await fetch(API+'/api/stats');
    const d=await r.json();
	  latestDbDate=d.max_date||'';
      const dateText = d.max_date ? `数据截止至: ${d.max_date}` : '暂无订单数据';
      const txtNode = document.getElementById('dbStatusText');
      if (txtNode) txtNode.textContent = dateText;
      const dot = document.getElementById('dot');
      if (dot) dot.className = 'bar-sep on';

	  // 填充商品下拉
	  const sel=document.getElementById('product');
	  const curVal=sel.value;
	  sel.innerHTML='<option value="">全部套餐</option>';
	  (d.products||[]).forEach(p=>{
	    const opt=document.createElement('option');
	    opt.value=p.name; opt.textContent=p.name+' ('+p.count+'单)';
	    if(p.name===curVal) opt.selected=true;
	    sel.appendChild(opt);
	  });
	  
	  // 渲染月度对账趋势柱状图（仅数据变化时重绘）
	  const chart = document.getElementById('monthlyChart');
	  if(chart && d.monthly && d.monthly.length) {
	    const filteredMonths = d.monthly.slice(-12); // 显示最近12个月
	    const chartKey = filteredMonths.map(m => m.month + ':' + m.fee_total).join('|');
	    if(chartKey !== window._lastChartKey) {
	      window._lastChartKey = chartKey;
	      const maxFee = Math.max(...filteredMonths.map(m => m.fee_total || 0), 1);
	      chart.innerHTML = filteredMonths.map(m => {
	        const fee = m.fee_total || 0;
	        const pct = Math.min(100, Math.round((fee / maxFee) * 100));
	        const monthLabel = m.month.includes('-') ? (parseInt(m.month.split('-')[1]) + '月') : m.month;
	        return `<div class="trend-col">
	          <div style="font-size: 9px; font-family: var(--font-mono); color: var(--text-muted); margin-bottom: 2px;">¥${Math.round(fee)}</div>
	          <div class="trend-bar-wrapper">
	            <div class="trend-bar-fill" style="height: 0%"></div>
	          </div>
	          <div class="trend-lbl">${monthLabel}</div>
	        </div>`;
	      }).join('');
	      setTimeout(() => {
	        const fills = chart.querySelectorAll('.trend-bar-fill');
	        filteredMonths.forEach((m, idx) => {
	          if(fills[idx]) {
	            const fee = m.fee_total || 0;
	            const pct = Math.min(100, Math.round((fee / maxFee) * 100));
	            fills[idx].style.height = pct + '%';
	          }
	        });
	      }, 50);
	    }
	  }
	  }catch(e){}
	}

// ═══ 年度统计图表系统 ═══


async function query(){
  if(window._isQuerying) return;
  window._isQuerying = true;
  const p=new URLSearchParams();
  const beginDate=getDT('start');
  const endDate=getDT('end');
  if(beginDate)p.set('beginDate',beginDate);
  if(endDate)p.set('endDate',endDate);
  
  const productInfo=document.getElementById('product').value.trim();
  if(productInfo)p.set('productInfo',productInfo);
  const couponValue=document.getElementById('coupon').value.trim();
  if(couponValue)p.set('couponValue',couponValue);
  const mobile=document.getElementById('mobile').value.trim();
  if(mobile)p.set('mobile',mobile);
  
  if(refundFilter==='normal')p.set('isRefunded','0');
  else if(refundFilter==='refunded')p.set('isRefunded','1');
  p.set('limit','5000');
	  const btn=document.getElementById('queryBtn');
	  btn.innerHTML='<span class="spin"></span>';
		  document.getElementById('tbody').innerHTML='<tr><td colspan="99" class="empty">查询中...</td></tr>';
	  document.getElementById('pager').innerHTML='';
	  try{
	    const r=await fetch(API+'/api/query?'+p.toString());
	    const d=await r.json();
	    allRows=d.rows||[];
	    curTotal=d.total||0;
	    curPage=1;
	    let sumFee=0, sumFinancial=0, refunded=0, colaCount=0;
	    allRows.forEach(r=>{
	      const feeObj=calcFee(r.product_info);
	      const fee=feeObj.fee;
	      const pi=r.product_info||"";
	      if(fee>0&&!r.is_refunded)sumFee+=fee;
	      if(fee>0&&!r.is_refunded){
	        const fin=calcFinancial(r.sale_price,r.discount_price);
	        if(fin>0)sumFinancial+=fin;
	      }
	      if(r.is_refunded)refunded++;
	      if(pi.includes("可乐"))colaCount++;
	    });
	    let timeStr = '';
	    if(allRows.length){
	      const dates=allRows.map(r=>r.consume_date||'').filter(Boolean).sort();
	      timeStr=dates[0]+' ~ '+dates[dates.length-1];
	    }else{
	      timeStr=((beginDate||'-')+' ~ '+(endDate||'-'));
	    }
	    let info='<div class="stat-capsule-group">';
	    info+=`<span class="stat-badge stat-badge-time"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" style="width:11px;height:11px"><rect x="3" y="4" width="18" height="18" rx="2" ry="2"></rect><line x1="16" y1="2" x2="16" y2="6"></line><line x1="8" y1="2" x2="8" y2="6"></line><line x1="3" y1="10" x2="21" y2="10"></line></svg>`+timeStr+`</span>`;
	    info+=`<span class="stat-badge stat-badge-total"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" style="width:11px;height:11px"><line x1="18" y1="20" x2="18" y2="10"></line><line x1="12" y1="20" x2="12" y2="4"></line><line x1="6" y1="20" x2="6" y2="14"></line></svg>共 `+curTotal+` 单</span>`;
	    if(colaCount>0) info+=`<span class="stat-badge stat-badge-cola"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" style="width:11px;height:11px"><polyline points="20 12 20 22 4 22 4 12"></polyline><rect x="2" y="7" width="20" height="5"></rect><line x1="12" y1="22" x2="12" y2="7"></line><path d="M12 7H7.5a2.5 2.5 0 0 1 0-5C11 2 12 7 12 7z"></path><path d="M12 7h4.5a2.5 2.5 0 0 0 0-5C13 2 12 7 12 7z"></path></svg>可乐 `+colaCount+`</span>`;
	    if(refunded>0) info+=`<span class="stat-badge stat-badge-refund"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" style="width:11px;height:11px"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path><line x1="12" y1="9" x2="12" y2="13"></line><line x1="12" y1="17" x2="12.01" y2="17"></line></svg>退款 `+refunded+`</span>`;
	    if(sumFee>0) info+=`<span class="stat-badge stat-badge-fee"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" style="width:11px;height:11px"><rect x="1" y="4" width="22" height="16" rx="2" ry="2"></rect><line x1="1" y1="10" x2="23" y2="10"></line></svg>计费 ¥`+sumFee.toLocaleString('zh-CN')+`</span>`;
	    if(sumFinancial>0) info+=`<span class="stat-badge stat-badge-fin"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" style="width:11px;height:11px"><polyline points="23 6 13.5 15.5 8.5 10.5 1 18"></polyline><polyline points="17 6 23 6 23 12"></polyline></svg>财务 ¥`+sumFinancial.toLocaleString('zh-CN',{minimumFractionDigits:2,maximumFractionDigits:2})+`</span>`;
	    info+='</div>';
	    document.getElementById('resultInfo').innerHTML=info;
	    document.getElementById('rangeInfo').textContent='';
	    if(!allRows.length){
	      document.getElementById('tbody').innerHTML='<tr><td colspan="99" class="empty">无数据</td></tr>';
    }else{
      renderTable();
    }
  }catch(e){
    document.getElementById('tbody').innerHTML='<tr><td colspan="99" class="empty">查询失败: '+esc(e.message)+'</td></tr>';
    showToast('数据查询失败: ' + e.message, 'error');
  }finally{
    btn.textContent='查询';
    window._isQuerying = false;
  }
}

function renderTable(){
  allRows.sort((a,b)=>{
    let va=(a[sortCol]||''),vb=(b[sortCol]||'');
    if(sortCol==='sale_price'){va=parseFloat(String(va).replace(/[¥￥,]/g,''))||0;vb=parseFloat(String(vb).replace(/[¥￥,]/g,''))||0}
    if(va<vb)return sortDir==='asc'?-1:1;
    if(va>vb)return sortDir==='asc'?1:-1;
    return 0;
  });
		  const kc=columnSettings.keyInfo;
		  const cs=columnSettings.columns;
		  const ci=id=>cs.find(x=>x.id===id);
		  const vis=id=>(ci(id)?.vis!==false);
		  // 更新表头显示
		  const ths=document.querySelectorAll('#dataTable th');
		  COLUMNS.forEach((c,idx)=>{if(ths[idx]) ths[idx].style.display=vis(c.id)?'':'none';});
		  const start=(curPage-1)*pageSize;
		  const pageRows=allRows.slice(0,curPage*pageSize);
		  document.getElementById('tbody').innerHTML=pageRows.map((row,i)=>{
		    const cls=row.is_refunded?'refunded':(row.product_info&&row.product_info.includes('可乐'))?'cola':'';
		    const badge=row.is_refunded?'<span class="badge badge-red">已退款</span>':'';
		    const idx=i;
		    const descVal=row.description?esc(row.description):'<span style="color:#ccc">—</span>';
		    const shopVal=row.shop_info?esc(row.shop_info):'<span style="color:#ccc">—</span>';
		    const fp=calcFee(row.product_info);
		    const fin=calcFinancial(row.sale_price,row.discount_price);
		    const fdTotal=getTotalDiscount(row.discount_price);
		    const feeText=fp.fee?`<span style="background:var(--plight);padding:2px 10px;border-radius:4px" title="由计费规则匹配自动折算">¥`+fp.fee+`</span>`:'<span style="color:#ccc">—</span>';
		    const finText=fin>0?`<span style="background:#fff7ed;padding:2px 10px;border-radius:4px;font-weight:600" title="计算公式: 销售价 - 商家优惠金额">¥`+fin.toFixed(2)+`</span>`:'<span style="color:#ccc">—</span>';
		    const cells=[];
		    if(vis('product_info')){const pi=row.product_info||'';cells.push('<td title="'+esc(pi)+'">'+esc(kc?getProductName(pi):pi)+'</td>');}
		    if(vis('product_type')) cells.push('<td>'+esc(row.product_type||'')+'</td>');
		    if(vis('coupon_value')) cells.push('<td>'+esc(row.coupon_value||'')+'</td>');
		    if(vis('sale_price')) cells.push('<td class="amount">'+fmtMoney(row.sale_price)+'</td>');
if(vis('discount_price')) cells.push('<td class="amount">'+(kc?(fdTotal>0?'¥'+fdTotal.toFixed(2):'—'):fmtMoney(row.discount_price))+'</td>');
	    if(vis('consume_date')) cells.push('<td class="time">'+esc(row.consume_date||'')+'</td>');
	    if(vis('mobile')) cells.push('<td class="phone" data-mobile="'+esc(row.mobile||'')+'">'+esc(maskPhone(row.mobile))+'</td>');
	    if(vis('description')) {
	        const descAttr = row.is_refunded ? ' style="color:var(--danger);font-weight:600"' : '';
	        cells.push('<td' + descAttr + '>' + descVal + '</td>');
	    }
	    if(vis('shop_info')) cells.push('<td>'+shopVal+'</td>');
	    if(vis('fee')) cells.push('<td style="font-weight:600;color:var(--danger)">'+feeText+'</td>');
	    if(vis('financial')) cells.push('<td class="fin-cell" style="font-family:\'SF Mono\',Consolas,monospace;font-weight:600;color:#c2410c">'+finText+'</td>');
	    return '<tr class="'+cls+'" data-coupon="'+esc(row.coupon_value||'')+'" onclick="showDetail('+idx+')">'+cells.join('')+'</tr>';
		  }).join('');
	  // 无限滚动：超过当前页则加载更多
	  if(curPage*pageSize<allRows.length){
	    document.getElementById('pager').innerHTML='<div style="text-align:center;padding:12px;color:var(--muted);font-size:12px;cursor:pointer" id="loadMore" onclick="loadMore()">加载更多...</div>';
	    // 滚动到底部自动加载
	    if(!window._scrollObs){
	      window._scrollObs=new IntersectionObserver(entries=>{
	        if(entries[0].isIntersecting&&curPage*pageSize<allRows.length) loadMore();
	      },{rootMargin:'200px'});
	    }
	    setTimeout(()=>{
	      const el=document.getElementById('loadMore');
	      if(el)window._scrollObs.observe(el);
	    },100);
	  }else{
	    document.getElementById('pager').innerHTML='';
	  }
	}
	function loadMore(){
	  curPage++;
	  renderTable();
	}

function showDetail(idx){
  const row=allRows[idx];
  if(!row)return;
  const fin=calcFinancial(row.sale_price,row.discount_price);
  const items=[
    ['券号',row.coupon_value],['交易快照',row.product_info],['商品类型',row.product_type],
    ['消费金额',row.sale_price],['商家优惠金额',row.discount_price],
    ['计费价',calcFee(row.product_info).fee||'-'],['财务价',fin>0?'¥'+fin.toFixed(2):'-'],
    ['消费时间',row.consume_date],
    ['用户手机',row.mobile],['备注',row.description],['验证门店',row.shop_info],
    ['退款状态',row.is_refunded?'已退款':'正常']
  ];
  document.getElementById('detailGrid').innerHTML=items.map(([k,v])=>
    '<div class="dk">'+esc(k)+'</div><div class="dv">'+esc(v||'-')+'</div>'
  ).join('');
  document.getElementById('detailModal').style.display='block';
}

function closeModal(id){document.getElementById(id).style.display='none'}

function exportCSV(){
  if(!allRows.length){
    showToast('无可导出的数据，请先执行查询', 'warning');
    return;
  }
  const headers=['交易快照','商品类型','券号','消费金额','商家优惠金额','财务价','计费价','消费时间','用户手机','备注','验证门店','退款'];
  const rows=allRows.map(r=>{
    const fin=calcFinancial(r.sale_price,r.discount_price);
    const fp=calcFee(r.product_info);
    return [
      r.product_info||'',r.product_type||'',r.coupon_value||'',
      r.sale_price||'',r.discount_price||'',fin>0?fin.toFixed(2):'0',
      fp.fee||'0',r.consume_date||'',
      r.mobile||'',r.description||'',r.shop_info||'',
      r.is_refunded?'是':'否'
    ];
  });
  const csv='\uFEFF'+headers.join(',')+'\n'+rows.map(r=>r.map(c=>'"'+String(c).replace(/"/g,'""')+'"').join(',')).join('\n');
  const blob=new Blob([csv],{type:'text/csv;charset=utf-8'});
  const a=document.createElement('a');
  a.href=URL.createObjectURL(blob);
  a.download='美团订单_'+fmt(new Date()).slice(0,10)+'.csv';
  a.click();
  showToast('订单数据已成功导出为 CSV 文件', 'success');
}

async function autoSync(){
  if(window._isSyncing) return;
  window._isSyncing = true;
  
  const btn=document.getElementById('refreshBtn');
  const badge=document.getElementById('newBadge');
  const t0 = Date.now();
  
  if(btn) btn.classList.add('active');
  document.getElementById('dot').className='bar-sep wait';
  
  try{
    const r=await fetch(API+'/api/refresh?deep=true');
    const d=await r.json();
    document.getElementById('dot').className='bar-sep on';
    await loadStats();
    
    if (d.new > 0) {
      await query(); // 仅在有新增订单时才静默重载订单表格，节省资源
      if (badge) {
        badge.textContent = `+${d.new}`;
        badge.className = 'show';
        
        // 3秒后红点气泡爆退消失
        setTimeout(() => {
          badge.className = '';
        }, 3000);
      }
    }
  }catch(e){
    document.getElementById('dot').className='bar-sep off';
  }finally{
    // 保证图标自转动画至少转满一圈 (800ms) 并 100% 释放锁
    const elapsed = Date.now() - t0;
    const delay = Math.max(0, 800 - elapsed);
    setTimeout(() => {
      if(btn) btn.classList.remove('active');
      window._isSyncing = false;
    }, delay);
  }
}

function startAuto(){
  if(autoTimer) clearInterval(autoTimer);
  // 首次运行
  autoSync();
  // 设定10秒 (10000ms) 高频自动同步守护任务
  autoTimer = setInterval(() => {
    if(document.getElementById('autoRefresh').checked) {
      autoSync();
    }
  }, 10000);
}

	
['detailModal', 'setModal', 'feeModal'].forEach(id => {
  const el = document.getElementById(id);
  if(el) {
    el.addEventListener('click', e => {
      if(e.target === e.currentTarget) closeModal(id);
    });
  }
});

document.querySelectorAll('.filters input').forEach(inp=>{
  inp.addEventListener('keydown',e=>{if(e.key==='Enter')query()});
});

document.getElementById('tbody').addEventListener('click',e=>{
  const td=e.target.closest('td.phone');
  if(td){
    e.stopPropagation();
    const full=td.dataset.mobile;
    if(full&&td.textContent!==full){td.textContent=full;td.style.color='var(--primary)'}
  }
});

async function init(){
  // 初始化计费配置(localStorage不存在或损坏时写入默认)
  if(!localStorage.getItem('feePlans')){
    localStorage.setItem('feePlans', JSON.stringify(DEFAULT_FEE_PLANS));
  }
  // 加载业务规则（班次 + 列显示）从后端，实现单源真实对齐（Single Source of Truth）
  try {
    const r = await fetch(API+'/api/settings');
    const s = await r.json();
    if(s){
      if(s.fee_json) try{window._feePlansCache=JSON.parse(s.fee_json);}catch(e){}
      if(s.shift){
        window._shiftSet={dayS:s.shift.day_start,dayE:s.shift.day_end,nightS:s.shift.night_start,nightE:s.shift.night_end};
      }
      if(s.columns){
        columnSettings = {keyInfo: s.key_info||false, copyHeader: s.copy_header||false, columns: s.columns};
        localStorage.setItem('columnSettings', JSON.stringify(columnSettings));
      }
    }
  } catch(e) {
    console.warn("Failed to fetch settings from backend, falling back to cache:", e);
  }
  // 加载本地缓存设置兜底
  const saved=localStorage.getItem('shiftSettings');
  if(saved){
    try{window._shiftSet=JSON.parse(saved)}catch(e){}
  }
  const h=new Date().getHours();
  if(h>=8&&h<20) setShift('day'); else setShift('night');
  try {
    await loadStats();
    await query(); // 初始加载时必须查询数据填充表格
  } catch(e) {
    console.error("loadStats failed:", e);
  }
  startAuto(); // 启动定时器（内部会再次调用autoSync）
  // 强行注销本地任何残留的历史 Service Worker 拦截以恢复纯净网络
  if ('serviceWorker' in navigator) {
    navigator.serviceWorker.getRegistrations().then(registrations => {
      for (let r of registrations) { r.unregister(); }
    });
  }

  // 页面重新可见时（从托盘切回）自动刷新
  document.addEventListener('visibilitychange',()=>{
    if(!document.hidden) autoSync();
  });
}

// 自定义时间时清除班次选中
document.querySelectorAll('#startDate,#startTime,#endDate,#endTime').forEach(inp=>{
  inp.addEventListener('change',clearShift);
});
// ══════════════════════════════════════════
//  一键复制大盘数据到剪贴板
// ══════════════════════════════════════════
function executeCopy(){
  if(!allRows || allRows.length === 0){
    showToast('当前无查询结果，无法复制', 'warning');
    return;
  }
  
  const selectedCols = COLUMNS.filter(c => {
    const colCfg = columnSettings.columns.find(x => x.id === c.id);
    return colCfg ? colCfg.copy !== false : c.pc;
  });
  
  if(selectedCols.length === 0){
    showToast('未预设任何复制列，请进入设置配置复制选项', 'warning');
    return;
  }
  
  // 拼接表头（如果设置了复制表头，则加上）
  let output = '';
  if (columnSettings.copyHeader) {
    output += selectedCols.map(c => c.label).join('\t') + '\n';
  }
  
  // 过滤掉已退款/已撤销的订单，不复制它们
  const copyRows = allRows.filter(r => !r.is_refunded);
  if(copyRows.length === 0){
    showToast('无可用正常订单复制（已退款/已撤销订单被自动过滤）', 'warning');
    return;
  }

  // 拼接每一行的数据
  copyRows.forEach(r => {
    const rowCells = selectedCols.map(c => {
      let val = '';
      if(c.id === 'fee'){
        const feeObj = calcFee(r.product_info);
        val = feeObj.fee > 0 ? '¥' + feeObj.fee.toFixed(2) : '-';
      } else if(c.id === 'financial'){
        if(r.is_refunded){
          val = '-';
        } else {
          const fin = calcFinancial(r.sale_price, r.discount_price);
          val = fin > 0 ? '¥' + fin.toFixed(2) : '¥0.00';
        }
      } else if(c.id === 'product_info'){
        val = columnSettings.keyInfo ? getProductName(r.product_info) : (r.product_info || '-');
      } else if(c.id === 'discount_price'){
        if (columnSettings.keyInfo) {
          const disc = getTotalDiscount(r.discount_price);
          val = disc > 0 ? '¥' + disc.toFixed(2) : '-';
        } else {
          val = r.discount_price || '-';
        }
      } else {
        val = r[c.id];
        if (val === null || val === undefined || val === '') {
          val = '-';
        }
      }
      return String(val).replace(/\r?\n|\r/g, " "); // 移走换行符
    });
    output += rowCells.join('\t') + '\n';
  });
  
  // 100% 兼容手机端局域网非 HTTPS 环境的写入剪切板方案
  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(output).then(() => {
      showToast('成功直接复制 ' + copyRows.length + ' 行数据，可直接粘贴到 Excel/Zoho 中（已排除已退款/撤销单）', 'success');
    }).catch(() => {
      fallbackCopy(output);
    });
  } else {
    fallbackCopy(output);
  }
}

function fallbackCopy(text) {
  try {
    const ta = document.createElement('textarea');
    ta.value = text;
    ta.style.position = 'fixed';
    ta.style.top = '0';
    ta.style.left = '0';
    ta.style.width = '2em';
    ta.style.height = '2em';
    ta.style.padding = '0';
    ta.style.border = 'none';
    ta.style.outline = 'none';
    ta.style.boxShadow = 'none';
    ta.style.background = 'transparent';
    document.body.appendChild(ta);
    ta.focus();
    ta.select();
    ta.setSelectionRange(0, 999999);
    
    const successful = document.execCommand('copy');
    document.body.removeChild(ta);
    if (successful) {
      showToast('已复制 ' + (allRows.filter(r => !r.is_refunded).length) + ' 行数据到剪贴板（已排除已退款/撤销单）', 'success');
    } else {
      showToast('您的设备不支持复制，请尝试在电脑端操作', 'warning');
    }
  } catch (err) {
    showToast('复制异常: ' + err.message, 'error');
  }
}

function openCopyModal(){
  executeCopy();
}

init().catch(err=>{
  console.error("Initialization warning:", err);
});
</script>

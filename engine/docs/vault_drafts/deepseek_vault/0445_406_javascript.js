async function ensureJSZip() {
if (typeof JSZip !== 'undefined') return;
return new Promise((resolve, reject) => {
GM_xmlhttpRequest({
method: "GET",
url: "https://cdnjs.cloudflare.com/ajax/libs/jszip/3.10.1/jszip.min.js",
onload: function(res) {
try {
eval(res.responseText);
console.log('[NLM] JSZip loaded via eval');
resolve();
} catch(e) { reject(e); }
},
onerror: reject
});
});
}
// Затем в exportAsZip перед созданием архива вызвать await ensureJSZip();

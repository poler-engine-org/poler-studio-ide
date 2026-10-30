// в функции exportAsZip
var zipLib = (typeof JSZip !== 'undefined' ? JSZip : (window.JSZip || null));
if (!zipLib) {
// fallback
}
var zip = new zipLib();

// ==UserScript==
// @name         Grok Files Downloader
// @match        https://grok.com/files*
// @grant        none
// ==/UserScript==

(function() {
setTimeout(() => {
const fileLinks = document.querySelectorAll('a[download], a[href*="/api/files/"]');
fileLinks.forEach(link => {
const url = link.href;
if (url && !link.hasAttribute('data-downloaded')) {
link.setAttribute('data-downloaded', 'true');
window.open(url, '_blank');
// або через fetch з blob
}
});
}, 3000);
})();

import {defineConfig} from 'vite';import tailwindcss from '@tailwindcss/vite';
export default defineConfig({plugins:[tailwindcss()],resolve:{alias:{'@':new URL('./src',import.meta.url).pathname}},clearScreen:false,server:{port:5173,strictPort:true},build:{target:['es2020','safari13'],cssTarget:'safari13'}});

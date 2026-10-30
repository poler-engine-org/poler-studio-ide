// В page.tsx
import dynamic from 'next/dynamic';

const FrederiteScene = dynamic(() => import('@/components/FrederiteScene'), {
ssr: false,
loading: () => <div>Loading 3D scene...</div>,
});

export default function Home() {
return (
<main>
<h1>3D Scene</h1>
<FrederiteScene />
</main>
);
}

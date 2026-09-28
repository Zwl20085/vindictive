import './styles/tokens.css';
import './styles/capture.css';
import { renderCapture } from './ui/capture';

const root = document.getElementById('capture');
if (!root) throw new Error('#capture root element missing');
renderCapture(root);

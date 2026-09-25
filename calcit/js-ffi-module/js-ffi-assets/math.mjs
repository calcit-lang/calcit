import { addOne } from './helper.mjs';

export const addTwo = (value) => addOne(addOne(value));

let count = 0;
export const nextCount = () => ++count;

const { compileLess } = require('../');

const source = `
@padding: 12px;
.card {
  padding: @padding;
  &:hover {
    padding: 16px;
  }
}
`;

const css = compileLess(source);
console.log(css);

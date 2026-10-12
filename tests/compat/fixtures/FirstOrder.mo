model FirstOrder
  Real x(start=1, fixed=true) "Dimensionless state";
equation
  der(x) = -x;
end FirstOrder;

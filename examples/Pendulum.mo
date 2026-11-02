model Pendulum "Planar pendulum"
  parameter Real m = 1;
  parameter Real L = 1;
  parameter Real g = 9.8;
  parameter Real J = m*L^2;
  Real phi(start = 0.1, fixed = true);
  Real w(start = 0, fixed = true);
equation
  der(phi) = w;
  J*der(w) = -m*g*L*sin(phi);
  annotation(experiment(StopTime = 10));
end Pendulum;

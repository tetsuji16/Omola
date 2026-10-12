model BouncingBall
  parameter Real gravity = 9.81 "Gravitational acceleration (m/s2)";
  parameter Real restitution = 0.7 "Velocity retention at impact";
  parameter Real initialHeight = 1 "Initial height (m)";

  Real height(start=initialHeight, fixed=true) "Height above the ground (m)";
  Real velocity(start=0, fixed=true) "Vertical velocity (m/s)";

equation
  der(height) = velocity;
  der(velocity) = -gravity;

  when height <= 0 then
    reinit(height, 0);
    reinit(velocity, -restitution * pre(velocity));
  end when;
end BouncingBall;

model MSLFirstOrder
  Modelica.Blocks.Sources.Step inputSignal(startTime=0.1);
  Modelica.Blocks.Continuous.FirstOrder plant(T=0.25, k=1);
equation
  connect(inputSignal.y, plant.u);
  annotation(uses(Modelica(version="4.1.0")));
end MSLFirstOrder;

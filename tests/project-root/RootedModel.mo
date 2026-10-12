model RootedModel
  Real height(start=1, fixed=true);
equation
  der(height) = -FixtureLib.gravity;
end RootedModel;
